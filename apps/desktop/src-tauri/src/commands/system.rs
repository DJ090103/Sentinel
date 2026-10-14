//! The `get_system_info` command.

use std::sync::{Arc, Mutex};

use sentinel_core::system::{SystemInfo, SystemMonitor};
use tauri::State;

/// Tauri-managed wrapper around the monitor.
///
/// The monitor has to outlive individual command invocations: CPU usage is a
/// delta between two readings, so a monitor constructed per call would have no
/// previous sample and would report 0% forever. Holding it in Tauri's state
/// means every poll measures against the one before it.
///
/// A `Mutex` because `snapshot` needs `&mut self` and Tauri may serve commands
/// from multiple threads. Contention is a non-issue — one caller, every couple
/// of seconds, holding the lock for microseconds.
///
/// The background snapshot job deliberately has its own monitor, so stored CPU
/// readings are interval averages rather than the dashboard's 2-second windows.
pub struct SystemMonitorState(pub Arc<Mutex<SystemMonitor>>);

impl SystemMonitorState {
    /// Builds the state, taking the monitor's initial baseline reading.
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(SystemMonitor::new())))
    }
}

impl Default for SystemMonitorState {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns a fresh reading of the host machine for the dashboard.
///
/// Thin by design, per `product-docs/ARCHITECTURE.md`: this does no measurement of its
/// own, it just unlocks the monitor and hands back what `sentinel-core`
/// produces.
#[tauri::command]
pub fn get_system_info(state: State<'_, SystemMonitorState>) -> SystemInfo {
    // Recovering from a poisoned lock rather than propagating the error: a
    // poisoned mutex here means some earlier snapshot panicked, but the monitor
    // holds only counters — nothing a panic can leave logically inconsistent.
    // Failing every future poll because of one past panic would be worse than
    // reading slightly odd CPU delta once.
    let mut monitor = state
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    monitor.snapshot()
}
