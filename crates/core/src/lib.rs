//! `sentinel-core` is the platform-agnostic heart of Sentinel.
//!
//! It knows nothing about Tauri, windows, or IPC. It exposes plain Rust
//! functions and types that `src-tauri` calls into. This separation means:
//!
//! - The domain logic can be unit tested with plain `cargo test`, no
//!   Tauri runtime or webview required.
//! - If DevHealth ever grows a CLI or a different frontend, this crate
//!   is reusable as-is.
//!
//! Modules below are organized by responsibility (Clean Architecture style),
//! not by technical layer. Most are currently empty shells — just doc
//! comments describing what will live there — and will be filled in one
//! at a time.

/// Gathers point-in-time facts about the host machine: CPU, RAM, disk,
/// OS, hostname. This is the data source for the dashboard's system cards.
pub mod system;

/// Defines the `Plugin` trait that every optimization target (npm, Docker,
/// Cursor, etc.) implements, plus the registry that holds active plugins.
/// No concrete plugins live here yet — only the contract.
pub mod plugins;

/// Coordinates running one or more plugins' `scan()` calls and collecting
/// their results into a unified report.
pub mod scanner;

/// Background job scheduling (Tokio-based). Runs recurring tasks — today
/// that's just a periodic system snapshot; later, periodic plugin scans.
pub mod scheduler;

/// SQLite connection setup and schema migrations.
pub mod database;

/// User-configurable app settings — theme, scheduler interval, last-scan
/// timestamp — persisted as a single JSON file at the OS config path.
/// Deliberately not backed by `database`; see `product-docs/ARCHITECTURE.md`.
pub mod settings;

/// In-app notification model — how the scheduler or scanner tells the UI
/// "something happened." Not OS-level notifications (yet).
pub mod notifications;

/// AI explanation layer. Trait definitions only — no model calls, no
/// inference. Deliberately inert until we're ready to build it.
pub mod ai;

/// Small shared helpers with no home of their own (formatting, error
/// conversion, etc.). Kept intentionally sparse to avoid becoming a
/// dumping ground.
pub mod utils;

/// Test doubles shared between this crate's unit tests. Never compiled into
/// a release build.
#[cfg(test)]
mod test_support;
