mod background;
mod commands;
mod plugins;

use background::SettingsState;
use commands::SystemMonitorState;
use plugins::PluginState;
use sentinel_core::settings::DEFAULT_SCREENSHOT_RETENTION_DAYS;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // Constructed once at startup rather than per invocation, so each CPU
        // reading is measured against the previous poll. See `commands::system`.
        .manage(SystemMonitorState::new())
        .setup(|app| {
            let monitor = app.state::<SystemMonitorState>().shared();

            match background::start(app.handle(), monitor) {
                // Held in managed state for the lifetime of the app: dropping
                // the handle aborts the scheduler, so letting it fall out of
                // scope here would stop the loop the moment setup returned.
                Ok(handle) => {
                    app.manage(handle);
                }
                // A degraded start, not a failed one. The dashboard reads the
                // machine live on its own timer and works without any of this;
                // refusing to open the window would help nobody.
                Err(cause) => eprintln!(
                    "Sentinel: background scheduling is disabled: {cause}. \
                     Live system readings are unaffected."
                ),
            }

            // Every plugin this build ships with. Built once at startup rather
            // than per scan: registration validates ids, and doing that on
            // every command would pay the cost repeatedly for an answer that
            // cannot change.
            //
            // Built *here*, after `background::start`, rather than in the
            // builder chain above: the junk plugin needs the screenshot
            // retention age from the settings file, and `SettingsState` does
            // not exist until the line above has run. When background startup
            // degraded there is no settings state, so the default applies —
            // one fewer configurable value, not a failed launch.
            let retention = app
                .try_state::<SettingsState>()
                .map(|settings| {
                    settings
                        .0
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .settings()
                        .screenshot_retention_days()
                })
                .unwrap_or(DEFAULT_SCREENSHOT_RETENTION_DAYS);

            app.manage(PluginState::new(retention));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::get_system_info,
            commands::scheduler::get_last_scan_at,
            commands::plugins::scan_plugins,
            commands::plugins::get_recommendations,
            commands::plugins::clean_targets
        ])
        .run(tauri::generate_context!())
        // Not an "infallible" expect: this is an unrecoverable startup failure.
        // If the webview or event loop won't start there is no degraded mode to
        // fall back to, and panicking surfaces the cause rather than exiting
        // silently with a zero status.
        .expect("Sentinel could not start its main window");
}
