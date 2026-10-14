//! Commands behind the Settings page's companion controls and the usage profile.

use sentinel_core::database::history::{self, UsageRow};
use sentinel_core::settings::companion::CompanionSettings;
use tauri::{AppHandle, Emitter, Manager};

use crate::background::{DatabaseState, SettingsState};
use crate::companion;

/// The current companion settings.
#[tauri::command]
pub fn get_companion_settings(app: AppHandle) -> Result<CompanionSettings, String> {
    companion_settings(&app)
}

fn companion_settings(app: &AppHandle) -> Result<CompanionSettings, String> {
    let state = app
        .try_state::<SettingsState>()
        .ok_or("settings are unavailable because the settings file could not be opened")?;
    let store = state.0.lock().unwrap_or_else(|p| p.into_inner());
    Ok(store.settings().companion.clone())
}

/// Validates, saves and applies new companion settings.
///
/// The webview's values are untrusted: they are sanitized (clamped, colours
/// checked) before they are stored or acted on, and the sanitized copy is what
/// is returned and broadcast.
#[tauri::command]
pub async fn set_companion_settings(
    app: AppHandle,
    settings: CompanionSettings,
) -> Result<CompanionSettings, String> {
    let state = app
        .try_state::<SettingsState>()
        .ok_or("settings are unavailable because the settings file could not be opened")?;

    let mut clean = settings.sanitized();
    {
        let mut store = state.0.lock().unwrap_or_else(|p| p.into_inner());
        // The window position is owned by dragging, not by the form.
        let existing = store.settings().companion.clone();
        clean.x = existing.x;
        clean.y = existing.y;
        let saved = clean.clone();
        store
            .update(|s| s.companion = saved)
            .map_err(|e| format!("the settings could not be saved: {e}"))?;
    }

    companion::apply(&app, &clean).map_err(|e| e.to_string())?;
    let _ = app.emit(companion::SETTINGS_EVENT, &clean);
    Ok(clean)
}

/// Called by the companion window after a drag so the position is remembered.
#[tauri::command]
pub fn companion_moved(app: AppHandle) -> Result<(), String> {
    let state = app
        .try_state::<SettingsState>()
        .ok_or("settings are unavailable")?;
    companion::remember_position(&app, &state.0)
}

/// The window's own close button: turns the companion off.
#[tauri::command]
pub async fn hide_companion(app: AppHandle) -> Result<(), String> {
    let mut cfg = companion_settings(&app)?;
    cfg.enabled = false;
    set_companion_settings(app, cfg).await.map(|_| ())
}

/// Active seconds per day, activity and app for the last `days` days.
#[tauri::command]
pub async fn get_usage(app: AppHandle, days: u32) -> Result<Vec<UsageRow>, String> {
    let Some(db) = app.try_state::<DatabaseState>() else {
        return Err("History is unavailable because the database could not be opened.".to_owned());
    };
    let conn = std::sync::Arc::clone(&db.0);
    let since = sentinel_core::activity::local_day_minus(days);

    tauri::async_runtime::spawn_blocking(move || {
        let conn = conn.lock().unwrap_or_else(|p| p.into_inner());
        history::usage_since(&conn, &since).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("usage could not be read: {e}"))?
}
