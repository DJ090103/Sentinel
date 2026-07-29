//! Starts Sentinel's background half and connects it to the webview.
//!
//! Three things happen here, in order: the settings file is loaded, the
//! scheduler is built over it with the snapshot job registered, and its feed of
//! completed runs is forwarded to the frontend as Tauri events.
//!
//! All of the *logic* lives in `sentinel_core` — this module is wiring, in
//! keeping with the rest of `src-tauri`. It exists because `core` cannot emit
//! Tauri events (see `product-docs/ARCHITECTURE.md` on why `core` has no Tauri
//! dependency), so something on this side has to bridge the two.
//!
//! Nothing here is allowed to stop the app launching. Sentinel without a
//! scheduler is a dashboard that still reads the machine live on a timer;
//! Sentinel that refuses to open is nothing at all.

use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use sentinel_core::notifications::{AppEvent, EVENT_NAME};
use sentinel_core::scheduler::snapshot::SystemSnapshotJob;
use sentinel_core::scheduler::{Scheduler, SchedulerHandle};
use sentinel_core::settings::{self, SettingsError, SettingsStore};
use sentinel_core::system::SystemMonitor;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::sync::broadcast::error::RecvError;

/// Why background scheduling couldn't be started.
///
/// Separate from [`SettingsError`] because the caller's decision differs: a
/// settings problem here doesn't fail the app, it disables one feature.
#[derive(Debug)]
pub enum StartupError {
    /// The settings file couldn't be located or read.
    Settings(SettingsError),
    /// A job couldn't be registered.
    Scheduler(sentinel_core::scheduler::SchedulerError),
}

impl fmt::Display for StartupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Settings(cause) => write!(formatter, "{cause}"),
            Self::Scheduler(cause) => write!(formatter, "{cause}"),
        }
    }
}

impl std::error::Error for StartupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Settings(cause) => Some(cause),
            Self::Scheduler(cause) => Some(cause),
        }
    }
}

impl From<SettingsError> for StartupError {
    fn from(cause: SettingsError) -> Self {
        Self::Settings(cause)
    }
}

impl From<sentinel_core::scheduler::SchedulerError> for StartupError {
    fn from(cause: sentinel_core::scheduler::SchedulerError) -> Self {
        Self::Scheduler(cause)
    }
}

/// The one settings store, shared between the snapshot job and any command
/// that needs to read what it recorded.
///
/// Managed by Tauri rather than constructed per caller: two stores over one
/// file would each hold their own copy of the settings and overwrite each
/// other's fields on the next write.
pub struct SettingsState(pub Arc<Mutex<SettingsStore>>);

/// Loads the settings, moving a file that doesn't parse aside rather than
/// overwriting it.
///
/// `SettingsStore` deliberately reports a corrupt file instead of silently
/// resetting, leaving the policy to the caller — this is that policy. The app
/// has to start, and the background job writes to this file on every tick, so
/// carrying on with defaults would destroy whatever the user had. Renaming it
/// keeps their file recoverable and lets the next write start clean.
///
/// Returns the store, plus the path the old file was preserved at when that
/// happened, so the caller can say so.
fn load_settings() -> Result<(SettingsStore, Option<PathBuf>), SettingsError> {
    let path = settings::default_settings_path()?;

    match SettingsStore::load_from(path.clone()) {
        Ok(store) => Ok((store, None)),
        Err(SettingsError::Parse(_)) => {
            // Timestamped, because `fs::rename` overwrites its destination
            // without complaint: a fixed name would mean the second corruption
            // silently destroyed what the first one preserved, which defeats
            // the entire point of keeping the file.
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let preserved = path.with_file_name(format!("settings.{stamp}.json.corrupt"));
            fs::rename(&path, &preserved)?;

            // Now that the unreadable file is out of the way, this is the
            // ordinary first-launch path and yields defaults.
            let store = SettingsStore::load_from(path)?;
            Ok((store, Some(preserved)))
        }
        Err(other) => Err(other),
    }
}

/// Loads settings, starts the scheduler, and forwards its runs to the webview.
///
/// The returned handle must be kept alive: dropping it aborts the loop. The
/// caller hands it to Tauri's managed state, which lives as long as the app.
pub fn start<R: Runtime>(
    app: &AppHandle<R>,
    monitor: Arc<Mutex<SystemMonitor>>,
) -> Result<SchedulerHandle, StartupError> {
    let (store, preserved) = load_settings()?;

    if let Some(path) = preserved {
        eprintln!(
            "Sentinel: settings file could not be read and was kept at {}; \
             starting from defaults",
            path.display()
        );
    }

    let interval = store.settings().scheduler_interval();
    let settings = Arc::new(Mutex::new(store));
    app.manage(SettingsState(Arc::clone(&settings)));

    let mut scheduler = Scheduler::new(interval);
    scheduler.register(Arc::new(SystemSnapshotJob::new(monitor, settings)))?;

    // `Scheduler::spawn` calls `tokio::spawn`, which panics unless a runtime is
    // entered. Tauri's setup hook doesn't run inside one, so borrow the handle
    // to the runtime Tauri already owns rather than standing up a second.
    let runtime = tauri::async_runtime::handle();
    let handle = {
        let _guard = runtime.inner().enter();
        scheduler.spawn()
    };

    forward_runs_to_frontend(app.clone(), &handle);

    Ok(handle)
}

/// Re-emits every completed job run to the webview as an [`AppEvent`].
///
/// The feed is live-only: a run that completes before the window has finished
/// loading is not replayed. That's why the dashboard reads its initial
/// "last scanned at" from the settings file rather than waiting for an event.
fn forward_runs_to_frontend<R: Runtime>(app: AppHandle<R>, handle: &SchedulerHandle) {
    let mut runs = handle.subscribe();

    tauri::async_runtime::spawn(async move {
        loop {
            match runs.recv().await {
                Ok(run) => {
                    if let Err(cause) = app.emit(EVENT_NAME, AppEvent::from(run)) {
                        // Emitting fails when there's no window to receive it,
                        // which is normal during shutdown and not worth
                        // stopping the loop over.
                        eprintln!("Sentinel: could not deliver a job event: {cause}");
                    }
                }
                // The UI fell behind by more than the channel's buffer. Only
                // the newest run matters for "last scanned at", so skipping the
                // ones in between costs nothing.
                Err(RecvError::Lagged(_)) => continue,
                // The scheduler has stopped; there will be no more runs.
                Err(RecvError::Closed) => break,
            }
        }
    });
}
