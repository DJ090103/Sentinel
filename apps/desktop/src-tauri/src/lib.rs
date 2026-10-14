mod background;
mod commands;
mod companion;
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
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        // Constructed once at startup rather than per invocation, so each CPU
        // reading is measured against the previous poll. See `commands::system`.
        .manage(SystemMonitorState::new())
        .manage(commands::analysis::AnalysisState::default())
        .setup(|app| {
            match background::start(app.handle()) {
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

            // The sensor loop runs from here on; the companion window appears
            // only if settings say it is enabled.
            companion::start(app.handle());
            if let Some(cfg) = app.try_state::<SettingsState>().map(|s| {
                s.0.lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .settings()
                    .companion
                    .clone()
            }) {
                if let Err(cause) = companion::apply(app.handle(), &cfg.sanitized()) {
                    eprintln!("Sentinel: could not open the companion: {cause}");
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::get_system_info,
            commands::scheduler::get_last_scan_at,
            commands::plugins::scan_plugins,
            commands::plugins::get_recommendations,
            commands::plugins::clean_targets,
            commands::analysis::analyze_home,
            commands::analysis::cancel_analysis,
            commands::insights::get_insights,
            commands::companion::get_companion_settings,
            commands::companion::set_companion_settings,
            commands::companion::companion_moved,
            commands::companion::hide_companion,
            commands::companion::get_usage
        ])
        .run(tauri::generate_context!())
        // Not an "infallible" expect: this is an unrecoverable startup failure.
        // If the webview or event loop won't start there is no degraded mode to
        // fall back to, and panicking surfaces the cause rather than exiting
        // silently with a zero status.
        .expect("Sentinel could not start its main window");
}
