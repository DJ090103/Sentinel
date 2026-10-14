//! Finding the three roots this plugin is allowed to look in.
//!
//! Each is resolved from the environment and then validated, exactly like
//! `plugins/browser`'s cache home. Nothing here accepts a path from a caller,
//! and every path a scan proposes is one of these roots plus compile-time
//! constant components.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use sentinel_plugin_api::{validate_tool_directory, PluginError};

/// Resolves the platform's cache home, which holds the thumbnail cache.
///
/// - Linux: `$XDG_CACHE_HOME`, defaulting to `~/.cache`
/// - macOS and Windows: unsupported — see [`screenshots`] and the crate docs.
pub fn cache_home() -> Result<PathBuf, PluginError> {
    cache_home_from(env::var_os("XDG_CACHE_HOME"), env::var_os("HOME"))
}

/// Resolves the platform's data home, which holds the trash.
///
/// - Linux: `$XDG_DATA_HOME`, defaulting to `~/.local/share`
/// - Everything else: unsupported.
pub fn data_home() -> Result<PathBuf, PluginError> {
    data_home_from(env::var_os("XDG_DATA_HOME"), env::var_os("HOME"))
}

/// Resolves the screenshots directory.
///
/// `~/Pictures/Screenshots`, which is where GNOME, KDE and most Linux desktops
/// put them.
///
/// # Why the XDG user-dirs file isn't consulted
///
/// `~/.config/user-dirs.dirs` can relocate `XDG_PICTURES_DIR`, and reading it
/// would cover more setups. It is deliberately not read: this is the one root
/// whose contents are the user's *own files* rather than a cache, and taking
/// the directory to walk from a config file would mean a stray edit in that
/// file could point this plugin at a directory full of work. A user whose
/// pictures live elsewhere gets "not applicable", which costs them a feature
/// they can do by hand. The alternative costs them files.
///
/// # macOS is refused outright
///
/// macOS screenshots default to `~/Desktop`. Offering to delete files from
/// someone's Desktop trades a structural guarantee for a promise not to make a
/// mistake, and this plugin isn't willing to make that trade — the same call
/// `plugins/browser` makes about Windows.
pub fn screenshots() -> Result<PathBuf, PluginError> {
    screenshots_from(env::var_os("HOME"))
}

/// Resolves the directories holding the user's temporary files.
///
/// - Linux: `/tmp` and `/var/tmp`
/// - Windows: `%TEMP%` (the per-user temp folder)
///
/// Only roots that exist and pass [`validate_tool_directory`] are returned.
/// Windows' machine-wide `C:\Windows\Temp` is deliberately absent: it needs
/// elevation and is handled separately.
pub fn temp_roots() -> Vec<PathBuf> {
    let candidates: Vec<PathBuf> = if cfg!(target_os = "linux") {
        vec![PathBuf::from("/tmp"), PathBuf::from("/var/tmp")]
    } else if cfg!(windows) {
        env::var_os("TEMP").map(PathBuf::from).into_iter().collect()
    } else {
        Vec::new()
    };

    candidates
        .into_iter()
        .filter(|path| validate_tool_directory(path).is_ok())
        .collect()
}

/// Resolves `%LOCALAPPDATA%`, which holds the Windows caches. `None` elsewhere.
pub fn local_app_data() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let path = env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    validate_tool_directory(&path).ok().map(|()| path)
}

fn cache_home_from(
    xdg_cache_home: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, PluginError> {
    if !cfg!(target_os = "linux") {
        return Err(unsupported_platform());
    }

    let candidate = xdg_cache_home
        .map(PathBuf::from)
        .or_else(|| home.map(|home| PathBuf::from(home).join(".cache")))
        .ok_or_else(|| PluginError::ToolUnavailable {
            detail: "no cache directory could be determined for this user".to_owned(),
        })?;

    validate_tool_directory(&candidate).map(|()| candidate)
}

fn data_home_from(
    xdg_data_home: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, PluginError> {
    if !cfg!(target_os = "linux") {
        return Err(unsupported_platform());
    }

    let candidate = xdg_data_home
        .map(PathBuf::from)
        .or_else(|| home.map(|home| PathBuf::from(home).join(".local").join("share")))
        .ok_or_else(|| PluginError::ToolUnavailable {
            detail: "no data directory could be determined for this user".to_owned(),
        })?;

    validate_tool_directory(&candidate).map(|()| candidate)
}

fn screenshots_from(home: Option<OsString>) -> Result<PathBuf, PluginError> {
    if cfg!(target_os = "macos") {
        return Err(PluginError::ToolUnavailable {
            detail: "screenshot cleanup is not supported on macOS, where screenshots are saved \
                     to the Desktop alongside the user's own files"
                .to_owned(),
        });
    }

    if !cfg!(target_os = "linux") {
        return Err(unsupported_platform());
    }

    let candidate = home
        .map(|home| PathBuf::from(home).join("Pictures").join("Screenshots"))
        .ok_or_else(|| PluginError::ToolUnavailable {
            detail: "no home directory could be determined for this user".to_owned(),
        })?;

    validate_tool_directory(&candidate).map(|()| candidate)
}

fn unsupported_platform() -> PluginError {
    PluginError::ToolUnavailable {
        detail: "system junk cleanup is currently implemented for Linux only".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(path: &std::path::Path) -> Option<OsString> {
        Some(path.as_os_str().to_owned())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cache_home_prefers_the_xdg_variable() {
        let cache = tempfile::tempdir().expect("temp dir");

        let resolved = cache_home_from(os(cache.path()), None).expect("resolves");

        assert_eq!(resolved, cache.path());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn data_home_falls_back_to_local_share() {
        let home = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(home.path().join(".local").join("share")).expect("create");

        let resolved = data_home_from(None, os(home.path())).expect("resolves");

        assert_eq!(resolved, home.path().join(".local").join("share"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn screenshots_is_a_named_child_of_pictures() {
        let home = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(home.path().join("Pictures").join("Screenshots")).expect("create");

        let resolved = screenshots_from(os(home.path())).expect("resolves");

        assert_eq!(
            resolved,
            home.path().join("Pictures").join("Screenshots"),
            "both components are constants, so no configuration can redirect this"
        );
    }

    /// A machine where these directories simply don't exist reads as "not
    /// applicable", not as a failure — the same call every other plugin makes
    /// about a tool that isn't installed.
    #[test]
    fn a_missing_directory_reads_as_not_applicable() {
        let empty = tempfile::tempdir().expect("temp dir");
        // A child that was never created. Pointing at the tempdir itself would
        // resolve successfully, since it exists.
        let absent = empty.path().join("not-created");

        for resolved in [
            cache_home_from(os(&absent), None),
            data_home_from(os(&absent), None),
            // `screenshots_from` takes HOME and appends Pictures/Screenshots,
            // so an existing tempdir with no such child is already the case
            // being tested.
            screenshots_from(os(empty.path())),
        ] {
            assert!(matches!(resolved, Err(PluginError::ToolUnavailable { .. })));
        }
    }

    #[test]
    fn no_environment_at_all_is_not_a_crash() {
        assert!(matches!(
            cache_home_from(None, None),
            Err(PluginError::ToolUnavailable { .. })
        ));
        assert!(matches!(
            screenshots_from(None),
            Err(PluginError::ToolUnavailable { .. })
        ));
    }
}
