//! Background job scheduling: a Tokio loop that runs registered jobs on a
//! recurring interval.
//!
//! The scheduler knows nothing about *what* it runs. It holds [`Job`] trait
//! objects, so the periodic system snapshot this exists for today and the
//! periodic plugin scans that come later slot in without changing anything
//! here. That indirection is the whole point of the module — see the Phase 4
//! guardrail in `product-docs/EXECUTION_PLAN.md`.
//!
//! Equally deliberately, there is no job priority, no retry policy, and no
//! backoff. One interval, jobs in registration order, failures recorded and
//! stepped over. Anything more is speculation until a second job type exists
//! to justify it.
//!
//! Jobs are *blocking*. Sentinel's work is filesystem-bound — walking cache
//! directories, stat-ing files — which is blocking I/O whatever syntax wraps
//! it (see `product-docs/ARCHITECTURE.md`, "The `Plugin` trait is synchronous"). Each
//! job therefore runs on `spawn_blocking` rather than on an async worker, so a
//! long scan can't stall the runtime.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

/// The periodic system snapshot: the one concrete [`Job`] that exists today.
///
/// Lives beside the runner rather than inside `system` because it is a piece of
/// *scheduling* — it exists only to be run on a timer, and `system` should stay
/// usable without one.
pub mod snapshot;

/// How many completed runs the broadcast channel buffers for a slow consumer.
///
/// Small on purpose: a subscriber that has fallen this far behind is not going
/// to catch up, and the useful thing to show a user is the latest run, not a
/// backlog of stale ones.
const RUN_HISTORY_BUFFER: usize = 16;

/// Whatever a job wants to report when it fails.
///
/// A boxed error rather than an enum of this module's own, because jobs are
/// supplied by callers: a snapshot job fails with a [`crate::settings`] error,
/// a future scan job with a plugin error, and this module has no business
/// enumerating them. `Send + Sync` because the error crosses back from a
/// blocking task.
pub type JobError = Box<dyn std::error::Error + Send + Sync>;

/// One unit of recurring background work.
///
/// Deliberately tiny: an identity and a thing to do. A job that needs
/// configuration or shared state captures it at construction, which keeps this
/// trait free of parameters that only one implementation would ever use.
pub trait Job: Send + Sync + 'static {
    /// Stable identifier, unique within a [`Scheduler`], e.g.
    /// `"system-snapshot"`. Used to label runs in the UI, so it should read
    /// like a name rather than a number.
    fn id(&self) -> &str;

    /// Does the work. Blocking — see the module docs.
    ///
    /// Returning `Err` records a failed run and the scheduler carries on to
    /// the next job. It must not be used to signal "stop scheduling me".
    fn run(&self) -> Result<(), JobError>;
}

/// How a single job run ended.
///
/// A tagged union so the frontend can match on `status` exhaustively, matching
/// [`crate::scanner::PluginScanStatus`]'s shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum JobOutcome {
    /// The job returned `Ok`.
    Succeeded,
    /// The job returned `Err`.
    ///
    /// Distinct from `Panicked`: a failure is usually environmental — a
    /// permission denied, a disk that filled up — and may well succeed on the
    /// next tick.
    Failed {
        /// Why, rendered through `Display` so the UI shows a sentence rather
        /// than a debug-formatted Rust type.
        detail: String,
    },
    /// The job panicked.
    ///
    /// Kept separate from `Failed` because this is a bug in the job, not a
    /// condition of the machine, and the two deserve different reactions from
    /// whoever reads the log.
    Panicked {
        /// The panic message, when it was a string payload.
        detail: String,
    },
}

/// A record of one job having run.
///
/// This is what the scheduler publishes; `crates/core/src/notifications` maps
/// it to something the UI renders, and the desktop shell forwards that over
/// Tauri's event system. Keeping the two apart means this module stays
/// testable without a frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRun {
    /// Which job, matching [`Job::id`].
    pub job_id: String,
    /// When it started.
    pub started_at: DateTime<Utc>,
    /// When it finished, successfully or not.
    pub finished_at: DateTime<Utc>,
    /// How it ended.
    pub outcome: JobOutcome,
}

impl JobRun {
    /// Whether this run did what it was supposed to.
    pub fn succeeded(&self) -> bool {
        matches!(self.outcome, JobOutcome::Succeeded)
    }
}

/// Everything that can go wrong setting a scheduler up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerError {
    /// Two jobs claimed the same [`Job::id`].
    ///
    /// Refused rather than tolerated, for the same reason
    /// [`crate::plugins::PluginRegistry`] refuses duplicate plugin ids: run
    /// records are labelled by id, and two jobs sharing one makes those
    /// records ambiguous exactly when someone is trying to debug them.
    DuplicateJobId {
        /// The id that was already taken.
        id: String,
    },
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateJobId { id } => {
                write!(formatter, "a job with id `{id}` is already registered")
            }
        }
    }
}

impl std::error::Error for SchedulerError {}

/// Jobs plus the interval they run on, before anything is running.
///
/// Split from [`SchedulerHandle`] so registration is finished before the loop
/// starts: a job added halfway through a tick would silently miss it, and the
/// type system rules that out instead of the documentation having to.
pub struct Scheduler {
    interval: Duration,
    jobs: Vec<Arc<dyn Job>>,
}

impl Scheduler {
    /// Builds an empty scheduler ticking at `interval`.
    ///
    /// Callers should pass [`crate::settings::Settings::scheduler_interval`]
    /// rather than a literal — it applies the floor that stops a hand-edited
    /// `0` in the settings file from becoming a busy loop. The interval is
    /// taken by value rather than read from the settings store because this
    /// module has no business owning the user's configuration.
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            jobs: Vec::new(),
        }
    }

    /// Adds a job, refusing an id that's already registered.
    pub fn register(&mut self, job: Arc<dyn Job>) -> Result<(), SchedulerError> {
        if self.jobs.iter().any(|existing| existing.id() == job.id()) {
            return Err(SchedulerError::DuplicateJobId {
                id: job.id().to_owned(),
            });
        }

        self.jobs.push(job);
        Ok(())
    }

    /// The registered job ids, in registration order.
    pub fn job_ids(&self) -> impl Iterator<Item = &str> {
        self.jobs.iter().map(|job| job.id())
    }

    /// How often the loop will tick.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Runs every job once, now, and reports how each one went.
    ///
    /// The tick body, exposed on its own so a caller can trigger a run outside
    /// the schedule — and so this module's behaviour under failing and
    /// panicking jobs can be tested without waiting on a clock.
    ///
    /// Jobs run one after another, not concurrently. They contend for the same
    /// disk, so overlapping them would trade predictable ordering for no real
    /// speedup.
    pub async fn run_once(&self) -> Vec<JobRun> {
        let mut runs = Vec::with_capacity(self.jobs.len());
        for job in &self.jobs {
            runs.push(run_job(Arc::clone(job)).await);
        }
        runs
    }

    /// Starts the loop on the current Tokio runtime.
    ///
    /// The first tick fires **immediately**, not one interval from now. That's
    /// deliberate: the job this exists for records when Sentinel last looked at
    /// the machine, and a five-minute window of "never scanned" right after
    /// launch would be an artifact of the scheduler rather than the truth.
    ///
    /// A tick that arrives while the previous one is still running is delayed
    /// rather than queued, so a slow run can't produce a burst of catch-up
    /// ticks the moment it finishes.
    ///
    /// Consumes `self`: see the type-level note on why registration closes
    /// before the loop opens.
    ///
    /// # Panics
    ///
    /// If called outside a Tokio runtime. That's `tokio::spawn`'s behaviour and
    /// it isn't worth converting to a `Result`: there is no runtime-less way to
    /// recover, and the caller is the desktop shell's `setup` hook, which
    /// always has one.
    pub fn spawn(self) -> SchedulerHandle {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let (runs_tx, _) = broadcast::channel(RUN_HISTORY_BUFFER);

        let publisher = runs_tx.clone();
        let interval = self.interval;

        let task = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        for run in self.run_once().await {
                            // A send with no subscribers is not a failure —
                            // nobody is listening yet, or the UI isn't open.
                            let _ = publisher.send(run);
                        }
                    }
                    _ = shutdown_rx.changed() => break,
                }
            }
        });

        SchedulerHandle {
            shutdown_tx,
            runs: runs_tx,
            task: Some(task),
        }
    }
}

/// A running scheduler.
///
/// **Dropping this stops the loop.** The handle owns the task, so letting it go
/// out of scope aborts the work rather than detaching it — a scheduler still
/// ticking after the thing that started it has gone is a leak that only shows
/// up as mysterious activity later. Hold it for as long as it should run, and
/// prefer [`SchedulerHandle::shutdown`] when a clean stop matters.
///
/// A drop stops the *loop*, not a job already in flight: aborting cancels at
/// the next await point, and a job handed to the blocking pool runs to
/// completion regardless. So a drop means "schedule nothing further", not
/// "stop what's happening right now" — which is the safer of the two for work
/// that may be partway through writing a file.
pub struct SchedulerHandle {
    shutdown_tx: watch::Sender<bool>,
    runs: broadcast::Sender<JobRun>,
    task: Option<JoinHandle<()>>,
}

impl SchedulerHandle {
    /// Subscribes to job runs as they complete.
    ///
    /// Only runs that happen *after* this call are delivered; the channel is a
    /// live feed, not a history. A subscriber that falls further than
    /// `RUN_HISTORY_BUFFER` behind loses the runs in between rather than
    /// slowing the scheduler down.
    pub fn subscribe(&self) -> broadcast::Receiver<JobRun> {
        self.runs.subscribe()
    }

    /// Stops the loop and waits for the in-flight tick to finish.
    ///
    /// Prefer this over dropping the handle when shutdown order matters — it
    /// lets a job that is midway through writing a file complete, where a drop
    /// would abort at the next await point.
    pub async fn shutdown(mut self) {
        // Fails only if the loop has already exited, which is the state being
        // asked for.
        let _ = self.shutdown_tx.send(true);

        if let Some(task) = self.task.take() {
            // A `JoinError` here means the loop panicked or was aborted; either
            // way it is no longer running, which is what this method promises.
            let _ = task.await;
        }
    }
}

impl Drop for SchedulerHandle {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

/// Runs one job on a blocking thread and records the result.
///
/// Panics are caught here rather than allowed to propagate: a job with a bug in
/// it must not take the scheduler — or the other jobs — down with it. This is
/// the same rule `scanner::scan_all` follows for a failing plugin.
async fn run_job(job: Arc<dyn Job>) -> JobRun {
    let job_id = job.id().to_owned();
    let started_at = Utc::now();

    let outcome = match tokio::task::spawn_blocking(move || job.run()).await {
        Ok(Ok(())) => JobOutcome::Succeeded,
        Ok(Err(cause)) => JobOutcome::Failed {
            detail: cause.to_string(),
        },
        Err(join_error) if join_error.is_panic() => JobOutcome::Panicked {
            detail: panic_detail(join_error.into_panic()),
        },
        // The remaining case is cancellation, which only happens if the
        // scheduler task was aborted mid-run — recorded rather than ignored so
        // a truncated run isn't mistaken for one that never started.
        Err(join_error) => JobOutcome::Failed {
            detail: join_error.to_string(),
        },
    };

    JobRun {
        job_id,
        started_at,
        finished_at: Utc::now(),
        outcome,
    }
}

/// Pulls a readable message out of a panic payload.
///
/// `panic!` payloads are `&str` or `String` in practice; anything else is
/// reported as unknown rather than debug-formatted, because a `Box<dyn Any>`
/// renders as nothing useful.
fn panic_detail(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "job panicked with a non-string payload".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::mpsc;

    /// A job whose behaviour each test picks, and which counts its own runs.
    struct TestJob {
        id: String,
        runs: Arc<AtomicUsize>,
        behaviour: Behaviour,
    }

    enum Behaviour {
        Succeed,
        Fail(&'static str),
        Panic(&'static str),
        /// Blocks for a while, so a test can arrange for shutdown to arrive
        /// while a run is genuinely in flight.
        Slow(Duration),
        /// Signals on a channel every time it runs, so a test can await a real
        /// tick instead of sleeping and hoping.
        Signal(mpsc::UnboundedSender<()>),
    }

    impl TestJob {
        fn new(id: &str, behaviour: Behaviour) -> (Arc<Self>, Arc<AtomicUsize>) {
            let runs = Arc::new(AtomicUsize::new(0));
            let job = Arc::new(Self {
                id: id.to_owned(),
                runs: Arc::clone(&runs),
                behaviour,
            });
            (job, runs)
        }
    }

    impl Job for TestJob {
        fn id(&self) -> &str {
            &self.id
        }

        fn run(&self) -> Result<(), JobError> {
            self.runs.fetch_add(1, Ordering::SeqCst);
            match &self.behaviour {
                Behaviour::Succeed => Ok(()),
                Behaviour::Fail(reason) => Err((*reason).into()),
                Behaviour::Panic(reason) => panic!("{reason}"),
                // A blocking sleep, not `tokio::time::sleep`: this runs on the
                // blocking pool, which is exactly what a real job does.
                Behaviour::Slow(duration) => {
                    std::thread::sleep(*duration);
                    Ok(())
                }
                Behaviour::Signal(sender) => {
                    let _ = sender.send(());
                    Ok(())
                }
            }
        }
    }

    fn a_minute() -> Duration {
        Duration::from_secs(60)
    }

    #[test]
    fn duplicate_job_ids_are_refused() {
        let (first, _) = TestJob::new("snapshot", Behaviour::Succeed);
        let (second, _) = TestJob::new("snapshot", Behaviour::Succeed);

        let mut scheduler = Scheduler::new(a_minute());
        scheduler.register(first).expect("first registration");

        let error = scheduler
            .register(second)
            .expect_err("a duplicate id must be refused");

        assert_eq!(
            error,
            SchedulerError::DuplicateJobId {
                id: "snapshot".to_owned()
            }
        );
        assert_eq!(scheduler.job_ids().count(), 1);
    }

    #[test]
    fn jobs_keep_their_registration_order() {
        let (first, _) = TestJob::new("alpha", Behaviour::Succeed);
        let (second, _) = TestJob::new("beta", Behaviour::Succeed);

        let mut scheduler = Scheduler::new(a_minute());
        scheduler.register(first).expect("register alpha");
        scheduler.register(second).expect("register beta");

        assert_eq!(scheduler.job_ids().collect::<Vec<_>>(), ["alpha", "beta"]);
    }

    /// The interval is meant to come from the settings file, and the floor that
    /// stops a hand-edited `0` becoming a busy loop lives on the settings side.
    /// This checks the two actually meet.
    #[test]
    fn the_interval_comes_from_settings_already_clamped() {
        use crate::settings::{Settings, MIN_SCHEDULER_INTERVAL_SECS};

        let defaults = Scheduler::new(Settings::default().scheduler_interval());
        assert_eq!(
            defaults.interval(),
            Duration::from_secs(crate::settings::DEFAULT_SCHEDULER_INTERVAL_SECS)
        );

        let hand_edited_to_zero = Settings {
            scheduler_interval_secs: 0,
            ..Settings::default()
        };
        let scheduler = Scheduler::new(hand_edited_to_zero.scheduler_interval());
        assert_eq!(
            scheduler.interval(),
            Duration::from_secs(MIN_SCHEDULER_INTERVAL_SECS),
            "a zero in the settings file must not reach the loop as a busy spin"
        );
    }

    #[tokio::test]
    async fn a_successful_run_is_recorded() {
        let (job, counter) = TestJob::new("snapshot", Behaviour::Succeed);
        let mut scheduler = Scheduler::new(a_minute());
        scheduler.register(job).expect("register");

        let runs = scheduler.run_once().await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].job_id, "snapshot");
        assert!(runs[0].succeeded());
        assert!(runs[0].finished_at >= runs[0].started_at);
    }

    /// The behaviour that matters most, mirroring `scanner`'s rule for a
    /// failing plugin: one bad job must not stop the others.
    #[tokio::test]
    async fn a_failing_job_does_not_stop_the_others() {
        let (broken, broken_counter) = TestJob::new("broken", Behaviour::Fail("disk on fire"));
        let (healthy, healthy_counter) = TestJob::new("healthy", Behaviour::Succeed);

        let mut scheduler = Scheduler::new(a_minute());
        scheduler.register(broken).expect("register broken");
        scheduler.register(healthy).expect("register healthy");

        let runs = scheduler.run_once().await;

        assert_eq!(broken_counter.load(Ordering::SeqCst), 1);
        assert_eq!(
            healthy_counter.load(Ordering::SeqCst),
            1,
            "the healthy job must still run"
        );
        assert_eq!(
            runs[0].outcome,
            JobOutcome::Failed {
                detail: "disk on fire".to_owned()
            }
        );
        assert!(runs[1].succeeded());
    }

    #[tokio::test]
    async fn a_panicking_job_is_contained() {
        let (exploding, _) = TestJob::new("exploding", Behaviour::Panic("index out of bounds"));
        let (healthy, healthy_counter) = TestJob::new("healthy", Behaviour::Succeed);

        let mut scheduler = Scheduler::new(a_minute());
        scheduler.register(exploding).expect("register exploding");
        scheduler.register(healthy).expect("register healthy");

        let runs = scheduler.run_once().await;

        assert_eq!(
            runs[0].outcome,
            JobOutcome::Panicked {
                detail: "index out of bounds".to_owned()
            },
            "a panic must be reported as a panic, not as a plain failure"
        );
        assert_eq!(
            healthy_counter.load(Ordering::SeqCst),
            1,
            "a panicking job must not stop the ones after it"
        );
    }

    #[tokio::test]
    async fn an_empty_scheduler_runs_cleanly() {
        let scheduler = Scheduler::new(a_minute());

        assert!(scheduler.run_once().await.is_empty());
    }

    /// The first tick has to be immediate — a fresh install would otherwise
    /// show "never scanned" for a full interval after launch.
    #[tokio::test]
    async fn the_first_tick_fires_without_waiting_for_the_interval() {
        let (sender, mut ticked) = mpsc::unbounded_channel();
        // An interval far longer than the test could ever wait, so anything
        // observed here can only have come from the immediate first tick.
        let (job, _) = TestJob::new("snapshot", Behaviour::Signal(sender));

        let mut scheduler = Scheduler::new(Duration::from_secs(3_600));
        scheduler.register(job).expect("register");
        let handle = scheduler.spawn();

        tokio::time::timeout(Duration::from_secs(5), ticked.recv())
            .await
            .expect("the first tick must not wait for the interval")
            .expect("the job signalled");

        handle.shutdown().await;
    }

    #[tokio::test]
    async fn runs_are_published_to_subscribers() {
        let (job, _) = TestJob::new("snapshot", Behaviour::Succeed);
        let mut scheduler = Scheduler::new(Duration::from_secs(3_600));
        scheduler.register(job).expect("register");

        let handle = scheduler.spawn();
        let mut runs = handle.subscribe();

        let run = tokio::time::timeout(Duration::from_secs(5), runs.recv())
            .await
            .expect("a run should be published promptly")
            .expect("the channel is open");

        assert_eq!(run.job_id, "snapshot");
        assert!(run.succeeded());

        handle.shutdown().await;
    }

    #[tokio::test]
    async fn a_repeating_interval_keeps_firing() {
        let (sender, mut ticked) = mpsc::unbounded_channel();
        let (job, counter) = TestJob::new("snapshot", Behaviour::Signal(sender));

        // Short enough that three ticks land well inside the timeout below.
        let mut scheduler = Scheduler::new(Duration::from_millis(50));
        scheduler.register(job).expect("register");
        let handle = scheduler.spawn();

        for tick in 1..=3 {
            tokio::time::timeout(Duration::from_secs(5), ticked.recv())
                .await
                .unwrap_or_else(|_| panic!("tick {tick} never arrived"))
                .expect("the job signalled");
        }

        handle.shutdown().await;
        assert!(counter.load(Ordering::SeqCst) >= 3);
    }

    #[tokio::test]
    async fn shutdown_stops_the_loop() {
        let (sender, mut ticked) = mpsc::unbounded_channel();
        let (job, counter) = TestJob::new("snapshot", Behaviour::Signal(sender));

        let mut scheduler = Scheduler::new(Duration::from_millis(50));
        scheduler.register(job).expect("register");
        let handle = scheduler.spawn();

        tokio::time::timeout(Duration::from_secs(5), ticked.recv())
            .await
            .expect("the first tick arrives")
            .expect("the job signalled");

        handle.shutdown().await;
        let after_shutdown = counter.load(Ordering::SeqCst);

        // Several intervals' worth of time in which a still-running loop would
        // certainly have ticked again.
        tokio::time::sleep(Duration::from_millis(300)).await;

        assert_eq!(
            counter.load(Ordering::SeqCst),
            after_shutdown,
            "the loop must not run after shutdown"
        );
    }

    /// `shutdown` promises to wait for the in-flight tick. Without that, a
    /// shutdown racing the tick body cancels it mid-run and the results are
    /// lost — invisible for a millisecond-long snapshot, ruinous for the
    /// multi-second plugin scans of Phase 5.
    #[tokio::test]
    async fn shutdown_waits_for_a_run_already_in_flight() {
        let (job, counter) = TestJob::new("slow", Behaviour::Slow(Duration::from_millis(200)));

        // Long interval, so the only run is the immediate first tick.
        let mut scheduler = Scheduler::new(Duration::from_secs(3_600));
        scheduler.register(job).expect("register");
        let handle = scheduler.spawn();
        let mut runs = handle.subscribe();

        // Long enough for the tick to have entered the job, short enough that
        // the job is certainly still running.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let requested_at = std::time::Instant::now();
        handle.shutdown().await;
        let waited = requested_at.elapsed();

        let run = tokio::time::timeout(Duration::from_secs(5), runs.recv())
            .await
            .expect("the in-flight run must be published, not abandoned")
            .expect("the run reached the channel");

        assert!(run.succeeded());
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "the job must have run to completion"
        );
        // The job had ~150ms left when shutdown was asked for. Returning sooner
        // than that would mean it abandoned the run rather than waiting for it.
        assert!(
            waited >= Duration::from_millis(100),
            "shutdown returned after only {waited:?}, so it did not wait for the run in flight"
        );
    }

    #[tokio::test]
    async fn dropping_the_handle_stops_the_loop() {
        let (sender, mut ticked) = mpsc::unbounded_channel();
        let (job, counter) = TestJob::new("snapshot", Behaviour::Signal(sender));

        let mut scheduler = Scheduler::new(Duration::from_millis(50));
        scheduler.register(job).expect("register");

        {
            let handle = scheduler.spawn();
            tokio::time::timeout(Duration::from_secs(5), ticked.recv())
                .await
                .expect("the first tick arrives")
                .expect("the job signalled");
            drop(handle);
        }

        // Let the abort take effect before sampling.
        tokio::time::sleep(Duration::from_millis(50)).await;
        let after_drop = counter.load(Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(300)).await;

        assert_eq!(
            counter.load(Ordering::SeqCst),
            after_drop,
            "dropping the handle must abort the loop rather than detach it"
        );
    }

    #[test]
    fn a_run_serializes_as_a_tagged_union_for_the_frontend() {
        let run = JobRun {
            job_id: "snapshot".to_owned(),
            started_at: Utc::now(),
            finished_at: Utc::now(),
            outcome: JobOutcome::Failed {
                detail: "nope".to_owned(),
            },
        };

        let json = serde_json::to_string(&run).expect("a run is plain data");

        assert!(json.contains("\"jobId\":\"snapshot\""), "got {json}");
        assert!(json.contains("\"status\":\"failed\""), "got {json}");
    }
}
