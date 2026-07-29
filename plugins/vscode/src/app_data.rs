//! Finding an Electron editor's application-data directory.
//!
//! No subprocess: these are Electron apps with a documented per-platform
//! location, and the editor binary has no "print your data directory" command
//! that works without launching a window.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use sentinel_plugin_api::{validate_tool_directory, PluginError};

/// Resolves the application-data directory for an editor.
///
/// Electron's `app.getPath('userData')` by platform:
///
/// - Linux: `$XDG_CONFIG_HOME/<name>`, defaulting to `~/.config/<name>`
/// - macOS: `~/Library/Application Support/<name>`
/// - Windows: `%APPDATA%\<name>`
///
/// `name` is a constant supplied by the plugin (`"Code"` or `"Cursor"`), never
/// user input, so the result is always a named child of a directory the OS
/// chose — the same protection the other plugins get from naming children.
pub fn resolve(name: &str) -> Result<PathBuf, PluginError> {
    resolve_from(
        name,
        env::var_os("XDG_CONFIG_HOME"),
        env::var_os("APPDATA"),
        env::var_os("HOME"),
    )
}

/// The resolution itself, over values already read from the environment.
///
/// Split out so tests can cover every platform's rule without mutating process
/// state, which cargo's parallel test threads would race on.
fn resolve_from(
    name: &str,
    xdg_config_home: Option<OsString>,
    appdata: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, PluginError> {
    let base = if cfg!(windows) {
        appdata.map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        home.map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
        })
    } else {
        xdg_config_home
            .map(PathBuf::from)
            .or_else(|| home.map(|home| PathBuf::from(home).join(".config")))
    };

    let base = base.ok_or_else(|| PluginError::ToolUnavailable {
        detail: format!("no application-data directory in which to look for {name}"),
    })?;

    let candidate = base.join(name);
    validate_tool_directory(&candidate).map(|()| candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(path: &std::path::Path) -> Option<OsString> {
        Some(path.as_os_str().to_owned())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn linux_prefers_xdg_config_home() {
        let config = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(config.path().join("Code")).expect("create");

        let resolved = resolve_from("Code", os(config.path()), None, None).expect("resolves");

        assert_eq!(resolved, config.path().join("Code"));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn linux_falls_back_to_dot_config_under_home() {
        let home = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(home.path().join(".config").join("Cursor")).expect("create");

        let resolved = resolve_from("Cursor", None, None, os(home.path())).expect("resolves");

        assert_eq!(resolved, home.path().join(".config").join("Cursor"));
    }

    #[test]
    fn an_editor_that_is_not_installed_reads_as_tool_unavailable() {
        let empty = tempfile::tempdir().expect("temp dir");

        assert!(matches!(
            resolve_from("Code", os(empty.path()), os(empty.path()), os(empty.path())),
            Err(PluginError::ToolUnavailable { .. })
        ));
    }

    #[test]
    fn no_environment_at_all_is_not_a_crash() {
        assert!(matches!(
            resolve_from("Code", None, None, None),
            Err(PluginError::ToolUnavailable { .. })
        ));
    }
}
