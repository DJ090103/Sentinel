//! Tauri commands: thin `#[tauri::command]` functions that the frontend
//! invokes over IPC. Each one should do as little as possible itself —
//! just deserialize input, call into `sentinel_core`, and serialize the
//! result. Business logic belongs in `core`, not here.

// Public so `tauri::generate_handler!` can name commands by their full path.
// The `#[tauri::command]` macro emits hidden helper items next to each command
// function, and a `pub use` of the function alone doesn't bring those along —
// the handler macro then can't find them.
pub mod analysis;
pub mod companion;
pub mod insights;
pub mod plugins;
pub mod scheduler;
pub mod system;

pub use system::SystemMonitorState;
