//! SQLite connection setup and schema migrations.
//!
//! This is the only place a database file is opened, so every caller gets a
//! connection that has already been migrated to the current schema. Callers
//! never run migrations themselves, which is what makes "open twice" safe.
//!
//! The first migration holds `system_snapshots` (one row per scheduler tick) and
//! `cleanup_history` (one row per plugin clean, mirroring `CleanOutcome`).

pub mod history;

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

/// Every schema change, in order. Append only: a shipped migration is never
/// edited, because users' files have already applied it.
static MIGRATIONS: LazyLock<Migrations<'static>> = LazyLock::new(|| {
    Migrations::new(vec![
        M::up(
            "CREATE TABLE system_snapshots (
            id INTEGER PRIMARY KEY,
            taken_at TEXT NOT NULL,
            cpu_percent REAL,
            ram_used_bytes INTEGER NOT NULL,
            ram_total_bytes INTEGER NOT NULL,
            disk_used_bytes INTEGER,
            disk_total_bytes INTEGER
        );
        CREATE INDEX idx_system_snapshots_taken_at ON system_snapshots(taken_at);
        CREATE TABLE cleanup_history (
            id INTEGER PRIMARY KEY,
            cleaned_at TEXT NOT NULL,
            plugin_id TEXT NOT NULL,
            removed_json TEXT NOT NULL,
            failure_count INTEGER NOT NULL,
            reclaimed_bytes INTEGER NOT NULL
        );
        CREATE INDEX idx_cleanup_history_cleaned_at ON cleanup_history(cleaned_at);",
        ),
        // Active seconds per local day, category and foreground app name. Only
        // names and categories: never window titles or content.
        M::up(
            "CREATE TABLE app_usage (
            day TEXT NOT NULL,
            activity TEXT NOT NULL,
            app TEXT NOT NULL,
            seconds INTEGER NOT NULL,
            PRIMARY KEY (day, activity, app)
        );",
        ),
    ])
});

/// Why a database could not be opened.
#[derive(Debug)]
pub enum DatabaseError {
    /// SQLite refused to open or configure the file.
    Sqlite(rusqlite::Error),
    /// A migration failed to apply.
    Migration(rusqlite_migration::Error),
}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "database error: {e}"),
            Self::Migration(e) => write!(f, "database migration failed: {e}"),
        }
    }
}

impl std::error::Error for DatabaseError {}

impl From<rusqlite::Error> for DatabaseError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

impl From<rusqlite_migration::Error> for DatabaseError {
    fn from(e: rusqlite_migration::Error) -> Self {
        Self::Migration(e)
    }
}

/// Opens (creating if needed) the database at `path` and migrates it to the
/// latest schema, so the rest of the app can assume the tables exist.
pub fn open(path: &Path) -> Result<Connection, DatabaseError> {
    if let Some(parent) = path.parent() {
        // Best effort: if this fails, `Connection::open` reports the real cause.
        let _ = std::fs::create_dir_all(parent);
    }
    migrated(Connection::open(path)?)
}

/// Where the database lives: the OS data directory, beside the settings file's
/// sibling `config` directory convention. `None` if the OS reports no home.
pub fn default_path() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "sentinel")?;
    Some(dirs.data_dir().join("sentinel.db"))
}

/// An in-memory database at the latest schema, for tests of code built on top.
pub fn open_in_memory() -> Result<Connection, DatabaseError> {
    migrated(Connection::open_in_memory()?)
}

fn migrated(mut conn: Connection) -> Result<Connection, DatabaseError> {
    MIGRATIONS.to_latest(&mut conn)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        MIGRATIONS.validate().expect("migration list must be valid");
    }

    #[test]
    fn opening_twice_is_idempotent() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("sentinel.db");
        open(&path).expect("first open");
        open(&path).expect("second open applies nothing and succeeds");
    }
}
