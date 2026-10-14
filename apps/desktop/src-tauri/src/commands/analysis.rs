//! The home-folder analysis command: staged progress in, a read-only report out.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use directories::BaseDirs;
use sentinel_core::analysis::{self, AnalysisError, HomeAnalysis, Progress};
use tauri::ipc::Channel;
use tauri::State;

/// The cancel flag of the analysis in flight, shared with `cancel_analysis`.
#[derive(Default)]
pub struct AnalysisState {
    cancel: Arc<AtomicBool>,
}

/// Walks the user's home folder and reports usage, large files and duplicates.
///
/// Runs only when the user presses the scan button. Progress streams over
/// `on_progress`; the report carries paths for display only — nothing here can
/// delete, and no path from the webview is ever accepted back.
#[tauri::command]
pub async fn analyze_home(
    state: State<'_, AnalysisState>,
    on_progress: Channel<Progress>,
) -> Result<HomeAnalysis, String> {
    let home = BaseDirs::new()
        .map(|dirs| dirs.home_dir().to_path_buf())
        .ok_or_else(|| "no home folder could be determined for this user".to_owned())?;

    let cancel = Arc::clone(&state.cancel);
    cancel.store(false, Ordering::Relaxed);

    tauri::async_runtime::spawn_blocking(move || {
        analysis::analyze_home(&home, &cancel, &mut |progress| {
            // A closed channel means the page went away; the walk finishing
            // is harmless, so the send failure is ignored.
            let _ = on_progress.send(progress);
        })
    })
    .await
    .map_err(|cause| format!("the analysis did not complete: {cause}"))?
    .map_err(|cause| match cause {
        AnalysisError::Cancelled => "cancelled".to_owned(),
        other => other.to_string(),
    })
}

/// Asks the running analysis to stop at its next checkpoint.
#[tauri::command]
pub fn cancel_analysis(state: State<'_, AnalysisState>) {
    state.cancel.store(true, Ordering::Relaxed);
}
