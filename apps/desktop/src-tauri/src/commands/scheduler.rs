//! The `get_last_scan_at` command.

use tauri::State;

use crate::background::SettingsState;

/// When the background scheduler last completed a run, as an RFC 3339 string.
///
/// `None` means it has never completed one — a first launch, or a machine where
/// background scheduling failed to start. The frontend renders that as "not
/// yet", which is true, rather than inventing a time.
///
/// # Why the frontend needs this at all
///
/// Runs are pushed to the UI as events, but that feed is live-only: the first
/// tick fires immediately at startup, well before the webview has finished
/// loading and subscribed, so the window almost always misses it. Without this
/// command the dashboard would show "not yet" for a full interval after every
/// launch despite a scan having just happened.
///
/// # Why a string rather than a timestamp type
///
/// Sending the RFC 3339 text keeps `src-tauri` free of a `chrono` dependency it
/// otherwise wouldn't need, and matches the wire format the event payload
/// already uses for `startedAt`/`finishedAt` — so the frontend parses one
/// format, not two.
#[tauri::command]
pub fn get_last_scan_at(state: State<'_, SettingsState>) -> Option<String> {
    // Recovering from a poisoned lock rather than failing the command: the
    // store holds plain settings values, and refusing to report the scan time
    // because some earlier write panicked helps nobody.
    let settings = state
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    settings
        .settings()
        .last_scan_at
        .map(|recorded| recorded.to_rfc3339())
}
