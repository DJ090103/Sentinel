//! What background work tells the frontend, and under what name.
//!
//! This module is the wire contract between Sentinel's background half and its
//! UI: the payload shape and the channel name, in one place, so the Rust side
//! and the TypeScript side have a single thing to agree on.
//!
//! It defines the message but does **not** send it. `crates/core` has no
//! dependency on Tauri (see `docs/ARCHITECTURE.md`), so the actual
//! `emit` lives in `apps/desktop/src-tauri`, which subscribes to the
//! scheduler's run feed and forwards each event using [`EVENT_NAME`]. Keeping
//! the type here means the contract is testable with `cargo test` and would
//! survive Sentinel growing a second frontend.
//!
//! Not OS-level toast notifications. Those are a separate concern, and nothing
//! here assumes a notification is ever shown to anyone — the dashboard's "last
//! scanned at" consumes these events without displaying a message at all.

use serde::Serialize;

use crate::scheduler::JobRun;

/// The Tauri event name every [`AppEvent`] is emitted under.
///
/// One channel for all app events rather than a name per event type: the
/// frontend registers a single listener and matches on the payload's tag, so
/// adding an event later doesn't require both sides to learn a new string.
///
/// Namespaced to avoid colliding with Tauri's own built-in event names.
pub const EVENT_NAME: &str = "sentinel://event";

/// Something the background half wants the UI to know about.
///
/// A tagged union, so the frontend can `switch` on `type` and ignore variants
/// it doesn't handle. That tag is the reason this is an enum today rather than
/// [`JobRun`] sent bare: a second event type is inevitable — a scan finishing,
/// a cleanup completing — and without a discriminator, adding one would be a
/// breaking change to a contract the frontend has already shipped against.
/// This is the same shape, for the same reason, as
/// [`crate::scanner::PluginScanStatus`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AppEvent {
    /// A scheduled job finished, successfully or not.
    ///
    /// Carries the whole [`JobRun`] rather than a summary string. The UI needs
    /// the timestamp to render "last scanned at ...", and needs the outcome to
    /// avoid claiming a successful scan when the job actually failed —
    /// flattening both into prose here would just mean parsing it back there.
    JobRan {
        /// How the run went.
        run: JobRun,
    },
}

impl AppEvent {
    /// Whether this event reports something that went wrong.
    ///
    /// Exists so a caller can decide whether an event deserves the user's
    /// attention without matching on every variant — the set of variants will
    /// grow, the question won't change.
    pub fn is_failure(&self) -> bool {
        match self {
            Self::JobRan { run } => !run.succeeded(),
        }
    }
}

impl From<JobRun> for AppEvent {
    fn from(run: JobRun) -> Self {
        Self::JobRan { run }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::JobOutcome;
    use chrono::Utc;

    fn run_with(outcome: JobOutcome) -> JobRun {
        JobRun {
            job_id: "system-snapshot".to_owned(),
            started_at: Utc::now(),
            finished_at: Utc::now(),
            outcome,
        }
    }

    /// The contract the frontend listener is written against. If this shape
    /// changes, TypeScript has to change with it.
    #[test]
    fn serializes_as_a_tagged_union_the_frontend_can_switch_on() {
        let event = AppEvent::from(run_with(JobOutcome::Succeeded));

        let json = serde_json::to_string(&event).expect("an event is plain data");

        assert!(json.contains(r#""type":"jobRan""#), "got {json}");
        assert!(json.contains(r#""jobId":"system-snapshot""#), "got {json}");
        assert!(json.contains(r#""status":"succeeded""#), "got {json}");
    }

    #[test]
    fn a_failed_run_is_reported_as_a_failure() {
        let failed = AppEvent::from(run_with(JobOutcome::Failed {
            detail: "disk full".to_owned(),
        }));
        let panicked = AppEvent::from(run_with(JobOutcome::Panicked {
            detail: "index out of bounds".to_owned(),
        }));
        let succeeded = AppEvent::from(run_with(JobOutcome::Succeeded));

        assert!(failed.is_failure());
        assert!(
            panicked.is_failure(),
            "a panicking job is a failure to whoever is watching"
        );
        assert!(!succeeded.is_failure());
    }

    /// Both sides of the IPC boundary hard-code this string, and only one of
    /// them is Rust.
    #[test]
    fn the_event_name_is_stable() {
        assert_eq!(EVENT_NAME, "sentinel://event");
    }
}
