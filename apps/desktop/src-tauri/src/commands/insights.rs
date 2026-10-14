//! The Insights command: stored history, read-only.

use sentinel_core::database::history::{self, Insights};
use tauri::{AppHandle, Manager};

use crate::background::DatabaseState;

/// Returns the last 30 days of readings and the cleanup history.
///
/// An error string means history is unavailable (no database); the page says so
/// instead of showing empty charts as if nothing had happened.
#[tauri::command]
pub async fn get_insights(app: AppHandle) -> Result<Insights, String> {
    let Some(db) = app.try_state::<DatabaseState>() else {
        return Err("History is unavailable because the database could not be opened.".to_owned());
    };
    let conn = std::sync::Arc::clone(&db.0);

    tauri::async_runtime::spawn_blocking(move || {
        let conn = conn.lock().unwrap_or_else(|p| p.into_inner());
        history::insights_now(&conn).map_err(|e| e.to_string())
    })
    .await
    .map_err(|cause| format!("history could not be read: {cause}"))?
}
