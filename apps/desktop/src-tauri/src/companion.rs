//! The desktop companion: its sensor loop, its window, and its start-at-login.
//!
//! All decisions (what counts as busy, which app is "coding") live in
//! `sentinel_core`; this module is the Tauri wiring: a thread that samples and
//! emits events, and the window that shows the result.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sentinel_core::activity::{self, Sampler};
use sentinel_core::database::history;
use sentinel_core::settings::companion::CompanionSettings;
use sentinel_core::settings::SettingsStore;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime, WebviewUrl};
use tauri::{WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;

use crate::background::{DatabaseState, SettingsState};

/// Carries each [`activity::Sample`] to every window.
pub const SAMPLE_EVENT: &str = "sentinel://sample";
/// Announces a settings change so the companion window restyles at once.
pub const SETTINGS_EVENT: &str = "sentinel://companion-settings";
/// The companion window's label.
pub const WINDOW_LABEL: &str = "companion";

/// Art-pixel canvas of the pixel monitor, and the name tag above it, in units
/// of `scale`. Mirrors `src/companion/dims.ts`.
const CANVAS_W: u32 = 40;
const CANVAS_H: u32 = 44;
const TAG_ROWS: u32 = 6;

fn current<R: Runtime>(app: &AppHandle<R>) -> Option<CompanionSettings> {
    let state = app.try_state::<SettingsState>()?;
    let store = state.0.lock().unwrap_or_else(|p| p.into_inner());
    Some(store.settings().companion.clone())
}

/// Starts the sensor thread. It runs for the life of the app.
///
/// Sampling is blocking (X11 round-trips, Win32 calls), so it gets its own
/// thread instead of occupying an async worker.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    std::thread::spawn(move || {
        let mut sampler = Sampler::new();
        let mut last = Instant::now();
        let mut carry_ms: u128 = 0;

        loop {
            let cfg = current(&app).unwrap_or_default().sanitized();
            let sample = sampler.sample(&cfg);

            carry_ms += last.elapsed().as_millis();
            last = Instant::now();
            let whole_secs = (carry_ms / 1000) as u64;

            // Usage is only recorded while foreground detection is on and the
            // user is actually active, and only as a category and app name.
            let active = sample
                .idle_sec
                .is_none_or(|idle| idle < u64::from(cfg.sleep_after_secs));
            if cfg.work_mode && active && whole_secs > 0 {
                carry_ms -= u128::from(whole_secs) * 1000;
                if let Some(db) = app.try_state::<DatabaseState>() {
                    let conn = db.0.lock().unwrap_or_else(|p| p.into_inner());
                    let _ = history::record_usage(
                        &conn,
                        &activity::local_day(),
                        sample.activity.map(activity::Activity::as_str),
                        sample.app.as_deref(),
                        whole_secs,
                    );
                }
            } else {
                carry_ms %= 1000;
            }

            let _ = app.emit(SAMPLE_EVENT, &sample);
            std::thread::sleep(Duration::from_millis(u64::from(cfg.poll_ms)));
        }
    });
}

fn size_for(scale: u32) -> (u32, u32) {
    (CANVAS_W * scale, (CANVAS_H + TAG_ROWS) * scale)
}

fn build_window<R: Runtime>(
    app: &AppHandle<R>,
    cfg: &CompanionSettings,
) -> tauri::Result<WebviewWindow<R>> {
    let (w, h) = size_for(cfg.scale);
    let window = WebviewWindowBuilder::new(
        app,
        WINDOW_LABEL,
        WebviewUrl::App("index.html#/companion".into()),
    )
    .title("Sentinel companion")
    .inner_size(f64::from(w), f64::from(h))
    .transparent(true)
    .decorations(false)
    .resizable(false)
    .shadow(false)
    .skip_taskbar(true)
    .always_on_top(cfg.always_on_top)
    .build()?;

    match (cfg.x, cfg.y) {
        (Some(x), Some(y)) => {
            window.set_position(PhysicalPosition::new(x, y))?;
        }
        // First show: the bottom-right corner of the primary monitor.
        _ => {
            if let Some(monitor) = window.primary_monitor()? {
                let size = monitor.size();
                let origin = monitor.position();
                let x = origin.x + size.width as i32 - w as i32 - 48;
                let y = origin.y + size.height as i32 - h as i32 - 96;
                window.set_position(PhysicalPosition::new(x, y))?;
            }
        }
    }
    Ok(window)
}

/// Makes the world match the settings: window shown or gone, size, stacking,
/// and start-at-login. Safe to call repeatedly.
pub fn apply<R: Runtime>(app: &AppHandle<R>, cfg: &CompanionSettings) -> tauri::Result<()> {
    match (app.get_webview_window(WINDOW_LABEL), cfg.enabled) {
        // Closed rather than hidden: a hidden window would keep the app alive
        // after the main window is closed.
        (Some(window), false) => window.close()?,
        (None, true) => {
            build_window(app, cfg)?;
        }
        (Some(window), true) => {
            let (w, h) = size_for(cfg.scale);
            window.set_size(tauri::LogicalSize::new(f64::from(w), f64::from(h)))?;
            window.set_always_on_top(cfg.always_on_top)?;
        }
        (None, false) => {}
    }

    // Autostart failures (unsupported desktop, missing permission) must not
    // fail the rest of the settings change.
    let autolaunch = app.autolaunch();
    let currently = autolaunch.is_enabled().unwrap_or(false);
    if cfg.launch_at_login != currently {
        let result = if cfg.launch_at_login {
            autolaunch.enable()
        } else {
            autolaunch.disable()
        };
        if let Err(cause) = result {
            eprintln!("Sentinel: could not change start at login: {cause}");
        }
    }
    Ok(())
}

/// Saves where the user dragged the companion, so it reopens there.
pub fn remember_position<R: Runtime>(
    app: &AppHandle<R>,
    store: &Arc<Mutex<SettingsStore>>,
) -> Result<(), String> {
    let window = app
        .get_webview_window(WINDOW_LABEL)
        .ok_or_else(|| "the companion is not open".to_owned())?;
    let position = window.outer_position().map_err(|e| e.to_string())?;
    store
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .update(|s| {
            s.companion.x = Some(position.x);
            s.companion.y = Some(position.y);
        })
        .map_err(|e| e.to_string())
}
