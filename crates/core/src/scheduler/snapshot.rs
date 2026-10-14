//! The periodic system snapshot — Sentinel's first real background job.
//!
//! It reads the machine and records *when* it did so in the settings file. The
//! reading itself is deliberately thrown away: there is nowhere to put a time
//! series until Phase 8 introduces SQLite (see `product-docs/ARCHITECTURE.md`,
//! "Persistence"), and a snapshot kept only in memory would be a stale copy of
//! something the dashboard already polls fresh every two seconds.
//!
//! Taking a reading that gets discarded is not busywork. `last_scan_at` claims
//! Sentinel looked at the machine at that moment, and the only way for that
//! claim to be true is to actually look. It also means the code path Phase 8
//! will persist is already running and already exercised, so that change
//! becomes "write the snapshot down" rather than "build the job".

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use rusqlite::Connection;

use crate::database::history;
use crate::scheduler::{Job, JobError};
use crate::settings::SettingsStore;
use crate::system::SystemMonitor;

/// The id this job registers under.
///
/// A constant rather than a string literal at each call site: the desktop shell
/// registers the job and the UI labels runs by id, and those two agreeing is
/// not something to leave to matching spelling.
pub const SYSTEM_SNAPSHOT_JOB_ID: &str = "system-snapshot";

/// Reads the machine on a schedule and records when it last did.
///
/// Both collaborators are shared rather than owned:
///
/// - The [`SettingsStore`] is the single writer of the settings file. A second
///   store over the same path would let two in-memory copies drift and
///   overwrite each other's fields.
/// - The [`SystemMonitor`] is shared so there is one refresh cycle on the
///   machine rather than two competing ones.
///
/// # The CPU figure's window is not fixed
///
/// [`SystemMonitor`] is stateful: CPU usage is the delta since the *previous*
/// refresh, whoever made it. Because the dashboard polls the same monitor every
/// two seconds while it is open, this job's reading is a two-second sample then
/// — but a full-interval average when no window is open. Same job, two
/// different meanings for the same number.
///
/// That is harmless today, because the reading is discarded. It stops being
/// harmless in Phase 8, which persists this figure into `system_snapshots` as
/// though it were one consistent metric. Resolving it — give this job its own
/// monitor and accept a consistent interval-average, or keep sharing and define
/// the column as "load sampled over an unspecified window" — is an open
/// question recorded in `product-docs/ARCHITECTURE.md`, to be settled before Phase 8
/// builds on it.
pub struct SystemSnapshotJob {
    monitor: Arc<Mutex<SystemMonitor>>,
    settings: Arc<Mutex<SettingsStore>>,
    /// Where readings are stored for Insights, if history is enabled.
    history: Option<Arc<Mutex<Connection>>>,
    /// False until the first run, whose CPU figure has nothing to measure against.
    primed: AtomicBool,
}

impl SystemSnapshotJob {
    /// Builds the job over the monitor and settings store it should share.
    pub fn new(monitor: Arc<Mutex<SystemMonitor>>, settings: Arc<Mutex<SettingsStore>>) -> Self {
        Self {
            monitor,
            settings,
            history: None,
            primed: AtomicBool::new(false),
        }
    }

    /// Also stores every reading in the history database.
    ///
    /// Pass a monitor of its own to [`new`](Self::new) when doing this, so the
    /// stored CPU figure is always an average over the job interval rather
    /// than whatever window the dashboard's polling happened to leave.
    pub fn with_history(mut self, history: Arc<Mutex<Connection>>) -> Self {
        self.history = Some(history);
        self
    }
}

impl Job for SystemSnapshotJob {
    fn id(&self) -> &str {
        SYSTEM_SNAPSHOT_JOB_ID
    }

    fn run(&self) -> Result<(), JobError> {
        // Scoped so the monitor lock is released before the settings lock is
        // taken. The two are never held at once, so no ordering between them
        // can deadlock against the dashboard command, which takes the monitor
        // lock on a different thread every couple of seconds.
        {
            // Recovering from a poisoned lock rather than failing: the monitor
            // holds counters, and there is no inconsistent state a panic could
            // leave behind. Refusing every future run because one earlier call
            // panicked would be the worse outcome.
            let mut monitor = self
                .monitor
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());

            let reading = monitor.snapshot();
            if let Some(db) = &self.history {
                // The first reading has no earlier sample, so its CPU is unknown.
                let measured = self.primed.swap(true, Ordering::Relaxed);
                let cpu = measured.then_some(reading.cpu.usage_percent);
                let conn = db.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                history::record_snapshot(&conn, &reading, cpu, Utc::now())?;
            }
        }

        let mut settings = self
            .settings
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        // `update` writes through, so there's no separate save to forget. A
        // failure here is a real failure of the job: the run happened, but
        // nothing would remember it after a restart.
        settings.update(|settings| settings.last_scan_at = Some(Utc::now()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::{JobOutcome, Scheduler};
    use crate::settings::SettingsError;
    use std::fs;
    use std::time::Duration;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("creating a temp dir cannot fail in a test environment")
    }

    fn job_over(path: std::path::PathBuf) -> (SystemSnapshotJob, Arc<Mutex<SettingsStore>>) {
        let store = SettingsStore::load_from(path).expect("a missing file loads as defaults");
        let settings = Arc::new(Mutex::new(store));
        let monitor = Arc::new(Mutex::new(SystemMonitor::new()));

        (
            SystemSnapshotJob::new(monitor, Arc::clone(&settings)),
            settings,
        )
    }

    #[test]
    fn a_run_records_when_it_happened() {
        let dir = temp_dir();
        let (job, settings) = job_over(dir.path().join("settings.json"));

        assert_eq!(
            settings
                .lock()
                .expect("uncontended in a single-threaded test")
                .settings()
                .last_scan_at,
            None,
            "nothing should have scanned yet"
        );

        let before = Utc::now();
        job.run().expect("the job succeeds against a temp dir");
        let after = Utc::now();

        let recorded = settings
            .lock()
            .expect("uncontended in a single-threaded test")
            .settings()
            .last_scan_at
            .expect("the run must record a timestamp");

        assert!(
            recorded >= before && recorded <= after,
            "timestamp {recorded} is outside the window the run happened in"
        );
    }

    /// The acceptance criterion this job exists for: the timestamp has to
    /// survive a restart, which means reaching the file and not just memory.
    #[test]
    fn the_timestamp_survives_reloading_from_disk() {
        let dir = temp_dir();
        let path = dir.path().join("settings.json");
        let (job, settings) = job_over(path.clone());

        job.run().expect("the job succeeds");

        let in_memory = settings
            .lock()
            .expect("uncontended")
            .settings()
            .last_scan_at
            .expect("recorded in memory");

        let reloaded = SettingsStore::load_from(path).expect("the file parses");

        assert_eq!(
            reloaded.settings().last_scan_at,
            Some(in_memory),
            "the timestamp must be on disk, not only in memory"
        );
    }

    #[test]
    fn each_run_advances_the_timestamp() {
        let dir = temp_dir();
        let (job, settings) = job_over(dir.path().join("settings.json"));

        job.run().expect("first run");
        let first = settings
            .lock()
            .expect("uncontended")
            .settings()
            .last_scan_at
            .expect("first timestamp");

        job.run().expect("second run");
        let second = settings
            .lock()
            .expect("uncontended")
            .settings()
            .last_scan_at
            .expect("second timestamp");

        assert!(
            second >= first,
            "a later run must not record an earlier time: {second} < {first}"
        );
    }

    #[test]
    fn other_settings_are_left_alone() {
        let dir = temp_dir();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"theme":"dark","schedulerIntervalSecs":900}"#)
            .expect("write existing settings");

        let (job, settings) = job_over(path);
        job.run().expect("the job succeeds");

        let guard = settings.lock().expect("uncontended");
        assert_eq!(guard.settings().theme, crate::settings::Theme::Dark);
        assert_eq!(
            guard.settings().scheduler_interval_secs,
            900,
            "recording a scan must not clobber the user's other preferences"
        );
    }

    /// A settings file that can't be written is a *failed* run, not a silent
    /// one — otherwise the dashboard would show a last-scan time that won't be
    /// there after a restart.
    #[test]
    fn a_failed_write_is_reported() {
        let dir = temp_dir();
        let blocked = dir.path().join("blocked");
        let path = blocked.join("settings.json");

        let (job, _settings) = job_over(path);

        // Put a *file* where the parent directory needs to be, so creating it
        // fails. Done after loading, since loading a missing file is the
        // first-launch case and has to succeed.
        fs::write(&blocked, "").expect("occupy the parent path with a file");

        let error = job
            .run()
            .expect_err("an unwritable settings file must fail");

        // Matched on the variant rather than the message, so rewording
        // `SettingsError`'s `Display` doesn't quietly turn this test green
        // against the wrong failure.
        let settings_error = error
            .downcast_ref::<SettingsError>()
            .unwrap_or_else(|| panic!("expected a settings failure, got: {error}"));

        assert!(
            matches!(settings_error, SettingsError::Io(_)),
            "expected an I/O failure, got: {settings_error:?}"
        );
    }

    /// End to end through the runner, since the point of this job is to be
    /// driven by the scheduler rather than called directly.
    #[tokio::test]
    async fn the_scheduler_runs_it_and_reports_success() {
        let dir = temp_dir();
        let (job, settings) = job_over(dir.path().join("settings.json"));

        let mut scheduler = Scheduler::new(Duration::from_secs(3_600));
        scheduler
            .register(Arc::new(job))
            .expect("registration succeeds");

        let runs = scheduler.run_once().await;

        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].job_id, SYSTEM_SNAPSHOT_JOB_ID);
        assert_eq!(runs[0].outcome, JobOutcome::Succeeded);
        assert!(settings
            .lock()
            .expect("uncontended")
            .settings()
            .last_scan_at
            .is_some());
    }

    #[test]
    fn the_job_id_is_stable() {
        let dir = temp_dir();
        let (job, _) = job_over(dir.path().join("settings.json"));

        // Registered by the desktop shell and rendered in the UI by this
        // string, so a rename is a breaking change rather than a tidy-up.
        assert_eq!(job.id(), "system-snapshot");
    }
}
