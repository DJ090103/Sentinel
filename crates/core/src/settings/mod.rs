//! User-configurable app settings, persisted as a single JSON file.
//!
//! Deliberately *not* a database. See `product-docs/ARCHITECTURE.md`, "Persistence":
//! this is a handful of small, flat values with no query or history
//! requirement, and keeping it as plain JSON at a documented path means a user
//! can read it, edit it, or delete it to reset the app without any tooling.
//!
//! The file is written through on every change, so there is no explicit "save"
//! step for a caller to forget.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

/// Interval the scheduler falls back to when the file doesn't specify one.
///
/// Five minutes: this drives a background snapshot, not the dashboard's live
/// cards (those poll every couple of seconds straight from the OS). There is
/// nothing to gain from snapshotting faster than the data meaningfully changes.
pub const DEFAULT_SCHEDULER_INTERVAL_SECS: u64 = 300;

/// Floor on the scheduler interval, whatever the file says.
///
/// The settings file is user-editable by design, so a `0` can end up in it — by
/// a typo, or by someone assuming the units are minutes. Without a floor that
/// becomes a busy loop that pins a core. Clamping beats rejecting the file,
/// which would lock someone out of their own app over one bad number.
pub const MIN_SCHEDULER_INTERVAL_SECS: u64 = 30;

/// Default age before a screenshot is offered for removal, in days.
pub const DEFAULT_SCREENSHOT_RETENTION_DAYS: u32 = 30;

/// Floor on the screenshot retention age, whatever the file says.
///
/// Same reasoning as [`MIN_SCHEDULER_INTERVAL_SECS`], with a worse failure
/// mode: this file is user-editable, and a `0` in it would otherwise mean
/// "offer every screenshot I have, including the one I took a minute ago".
/// These are the user's own files, not a cache, so the floor is not optional.
pub const MIN_SCREENSHOT_RETENTION_DAYS: u32 = 7;

/// Which colour scheme the UI should use.
///
/// `System` defers to the OS rather than pinning a value. It's the default, and
/// it's why this is a three-state enum rather than a boolean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Always light.
    Light,
    /// Always dark.
    Dark,
    /// Follow the operating system's preference.
    #[default]
    System,
}

/// Everything Sentinel remembers between launches.
///
/// `#[serde(default)]` on the struct means a file written by an older version —
/// or hand-edited down to a single key — still loads, with absent fields taking
/// their default. Adding a field must never invalidate an existing user's file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// UI colour scheme.
    pub theme: Theme,
    /// How often the background scheduler runs, in seconds.
    ///
    /// Read this through [`Settings::scheduler_interval`] rather than directly;
    /// the accessor applies [`MIN_SCHEDULER_INTERVAL_SECS`].
    pub scheduler_interval_secs: u64,
    /// When the scheduler last completed a run, or `None` if it never has.
    pub last_scan_at: Option<DateTime<Utc>>,
    /// How old a screenshot must be before System Scan offers it, in days.
    ///
    /// Read this through [`Settings::screenshot_retention_days`] rather than
    /// directly; the accessor applies [`MIN_SCREENSHOT_RETENTION_DAYS`].
    ///
    /// There is no UI control for this yet — it is edited in this file, which
    /// is a supported way to configure Sentinel (see the module docs).
    pub screenshot_retention_days: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            scheduler_interval_secs: DEFAULT_SCHEDULER_INTERVAL_SECS,
            last_scan_at: None,
            screenshot_retention_days: DEFAULT_SCREENSHOT_RETENTION_DAYS,
        }
    }
}

impl Settings {
    /// The scheduler interval as a `Duration`, clamped to a sane floor.
    ///
    /// Always prefer this over reading `scheduler_interval_secs` directly — see
    /// [`MIN_SCHEDULER_INTERVAL_SECS`] for why the raw value isn't trustworthy.
    pub fn scheduler_interval(&self) -> Duration {
        Duration::from_secs(
            self.scheduler_interval_secs
                .max(MIN_SCHEDULER_INTERVAL_SECS),
        )
    }

    /// The screenshot retention age, clamped to a sane floor.
    ///
    /// Always prefer this over reading `screenshot_retention_days` directly —
    /// see [`MIN_SCREENSHOT_RETENTION_DAYS`] for why the raw value isn't
    /// trustworthy. The junk plugin applies the same floor independently, so
    /// neither layer relies on the other having remembered.
    pub fn screenshot_retention_days(&self) -> u32 {
        self.screenshot_retention_days
            .max(MIN_SCREENSHOT_RETENTION_DAYS)
    }
}

/// Everything that can go wrong reading or writing the settings file.
///
/// A missing file is deliberately *not* in here: that's the first-launch case,
/// and it resolves to [`Settings::default`] rather than an error.
#[derive(Debug)]
pub enum SettingsError {
    /// The OS didn't give us a config directory to write into.
    NoConfigDir,
    /// The file couldn't be read or written.
    Io(io::Error),
    /// The file exists and is readable, but isn't valid settings JSON.
    ///
    /// Surfaced rather than silently replaced: overwriting a file we failed to
    /// understand would throw away configuration the user may have written by
    /// hand. The caller decides whether to fall back to defaults.
    Parse(serde_json::Error),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoConfigDir => write!(formatter, "could not determine an OS config directory"),
            Self::Io(cause) => write!(formatter, "settings file I/O failed: {cause}"),
            Self::Parse(cause) => write!(formatter, "settings file is not valid JSON: {cause}"),
        }
    }
}

impl std::error::Error for SettingsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NoConfigDir => None,
            Self::Io(cause) => Some(cause),
            Self::Parse(cause) => Some(cause),
        }
    }
}

impl From<io::Error> for SettingsError {
    fn from(cause: io::Error) -> Self {
        Self::Io(cause)
    }
}

/// The OS-appropriate path to `settings.json`.
///
/// `~/.config/sentinel/settings.json` on Linux, with the platform equivalents
/// elsewhere. Resolved via the `directories` crate rather than hand-rolled, so
/// `XDG_CONFIG_HOME` overrides and the macOS/Windows conventions are honoured
/// instead of reinvented.
pub fn default_settings_path() -> Result<PathBuf, SettingsError> {
    let dirs = ProjectDirs::from("", "", "sentinel").ok_or(SettingsError::NoConfigDir)?;
    Ok(dirs.config_dir().join("settings.json"))
}

/// Owns the settings and keeps the file on disk in step with them.
///
/// Holds the values in memory so reads are free, and writes through on every
/// mutation so a crash can't lose a change the user already saw take effect.
#[derive(Debug)]
pub struct SettingsStore {
    path: PathBuf,
    settings: Settings,
}

impl SettingsStore {
    /// Loads from the OS-appropriate location.
    pub fn load() -> Result<Self, SettingsError> {
        Self::load_from(default_settings_path()?)
    }

    /// Loads from an explicit path.
    ///
    /// Exists so tests can run against a temporary directory: a test suite that
    /// writes to the developer's real `~/.config` isn't one you can run safely.
    ///
    /// A missing file is the first-launch case and yields defaults without
    /// touching the disk. A file that exists but doesn't parse is an error, not
    /// a silent reset.
    pub fn load_from(path: PathBuf) -> Result<Self, SettingsError> {
        let settings = match fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).map_err(SettingsError::Parse)?,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => Settings::default(),
            Err(cause) => return Err(SettingsError::Io(cause)),
        };

        Ok(Self { path, settings })
    }

    /// The current settings.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Where this store reads and writes.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Mutates the settings and immediately persists them.
    ///
    /// Takes a closure rather than handing out `&mut Settings` so there is no
    /// way to change a value without the write happening — the "forgot to save"
    /// bug is unrepresentable.
    ///
    /// If the write fails the in-memory value is still updated: the user asked
    /// for the change and the running app should honour it, with the error
    /// surfaced so the caller can say it won't survive a restart.
    pub fn update<F>(&mut self, mutate: F) -> Result<(), SettingsError>
    where
        F: FnOnce(&mut Settings),
    {
        mutate(&mut self.settings);
        self.persist()
    }

    /// Writes the current settings to disk, creating parent directories.
    ///
    /// Writes a temporary file in the same directory and renames it over the
    /// target. `rename` within a filesystem is atomic, so a crash or a full disk
    /// mid-write leaves the previous file intact rather than a truncated one. A
    /// half-written settings file would fail to parse on next launch, which is
    /// exactly the failure this module most needs to avoid causing.
    fn persist(&self) -> Result<(), SettingsError> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other("settings path has no parent directory"))?;
        fs::create_dir_all(parent)?;

        let json = serde_json::to_string_pretty(&self.settings).map_err(SettingsError::Parse)?;

        let temp_path = self.path.with_extension("json.tmp");
        fs::write(&temp_path, json)?;

        // Remove the temporary file if the rename fails, so a failed write
        // doesn't leave litter next to the real one.
        if let Err(cause) = fs::rename(&temp_path, &self.path) {
            let _ = fs::remove_file(&temp_path);
            return Err(SettingsError::Io(cause));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("creating a temp dir cannot fail in a test environment")
    }

    #[test]
    fn missing_file_falls_back_to_defaults_without_writing() {
        let dir = temp_dir();
        let path = dir.path().join("settings.json");

        let store = SettingsStore::load_from(path.clone()).expect("missing file is not an error");

        assert_eq!(store.settings(), &Settings::default());
        assert!(!path.exists(), "loading must not create the file");
    }

    #[test]
    fn changes_round_trip_through_the_file() {
        let dir = temp_dir();
        let path = dir.path().join("settings.json");
        let timestamp = Utc::now();

        let mut store = SettingsStore::load_from(path.clone()).expect("load defaults");
        store
            .update(|settings| {
                settings.theme = Theme::Dark;
                settings.scheduler_interval_secs = 600;
                settings.last_scan_at = Some(timestamp);
            })
            .expect("writing into a temp dir succeeds");

        let reloaded = SettingsStore::load_from(path).expect("reload the written file");

        assert_eq!(reloaded.settings().theme, Theme::Dark);
        assert_eq!(reloaded.settings().scheduler_interval_secs, 600);
        assert_eq!(reloaded.settings().last_scan_at, Some(timestamp));
    }

    #[test]
    fn update_persists_immediately_with_no_explicit_save() {
        let dir = temp_dir();
        let path = dir.path().join("settings.json");

        let mut store = SettingsStore::load_from(path.clone()).expect("load defaults");
        store
            .update(|settings| settings.theme = Theme::Light)
            .expect("write succeeds");

        let raw = fs::read_to_string(&path).expect("file exists straight after update");
        assert!(raw.contains("\"light\""), "got {raw}");
    }

    #[test]
    fn corrupt_file_is_reported_not_silently_reset() {
        let dir = temp_dir();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{ not json at all ").expect("write a corrupt file");

        let error = SettingsStore::load_from(path).expect_err("corrupt file must not load");

        assert!(
            matches!(error, SettingsError::Parse(_)),
            "expected a parse error, got {error:?}"
        );
        assert!(error.to_string().contains("not valid JSON"), "{error}");
    }

    #[test]
    fn partial_file_keeps_defaults_for_absent_fields() {
        let dir = temp_dir();
        let path = dir.path().join("settings.json");
        // A file written by a version that only knew about `theme`.
        fs::write(&path, r#"{"theme":"dark"}"#).expect("write a partial file");

        let store = SettingsStore::load_from(path).expect("a partial file still loads");

        assert_eq!(store.settings().theme, Theme::Dark);
        assert_eq!(
            store.settings().scheduler_interval_secs,
            DEFAULT_SCHEDULER_INTERVAL_SECS
        );
        assert_eq!(store.settings().last_scan_at, None);
    }

    #[test]
    fn unknown_fields_do_not_break_loading() {
        let dir = temp_dir();
        let path = dir.path().join("settings.json");
        // A file written by a *newer* version than this one.
        fs::write(&path, r#"{"theme":"light","somethingFromTheFuture":42}"#)
            .expect("write a forward-compatible file");

        let store = SettingsStore::load_from(path).expect("unknown keys are ignored");

        assert_eq!(store.settings().theme, Theme::Light);
    }

    #[test]
    fn scheduler_interval_is_clamped_to_a_floor() {
        let busy_loop = Settings {
            scheduler_interval_secs: 0,
            ..Settings::default()
        };
        assert_eq!(
            busy_loop.scheduler_interval(),
            Duration::from_secs(MIN_SCHEDULER_INTERVAL_SECS),
            "a zero interval must not become a busy loop"
        );

        let explicit = Settings {
            scheduler_interval_secs: 900,
            ..Settings::default()
        };
        assert_eq!(explicit.scheduler_interval(), Duration::from_secs(900));
    }

    #[test]
    fn persist_leaves_no_temporary_file_behind() {
        let dir = temp_dir();
        let path = dir.path().join("settings.json");

        let mut store = SettingsStore::load_from(path).expect("load defaults");
        store
            .update(|settings| settings.theme = Theme::Dark)
            .expect("write succeeds");

        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .expect("temp dir is readable")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();

        assert!(
            leftovers.is_empty(),
            "left temp files behind: {leftovers:?}"
        );
    }

    #[test]
    fn creates_missing_parent_directories() {
        let dir = temp_dir();
        // Two levels that don't exist yet — the real first-launch case, where
        // `~/.config/sentinel/` has never been created.
        let path = dir
            .path()
            .join("nested")
            .join("sentinel")
            .join("settings.json");

        let mut store = SettingsStore::load_from(path.clone()).expect("load defaults");
        store
            .update(|settings| settings.theme = Theme::Dark)
            .expect("write creates parent directories");

        assert!(path.exists(), "settings file was not created");
    }

    #[test]
    fn default_path_sits_under_a_sentinel_config_directory() {
        // Returns early rather than failing where there's no home directory:
        // that's a property of the environment, not a defect in this code.
        let Ok(path) = default_settings_path() else {
            return;
        };

        assert!(path.ends_with("sentinel/settings.json"), "got {path:?}");
    }

    #[test]
    fn serializes_to_camel_case_for_the_frontend() {
        let json =
            serde_json::to_string(&Settings::default()).expect("Settings contains only plain data");

        assert!(json.contains("\"theme\":\"system\""), "got {json}");
        assert!(json.contains("\"schedulerIntervalSecs\":300"), "got {json}");
        assert!(json.contains("\"lastScanAt\":null"), "got {json}");
    }
}
