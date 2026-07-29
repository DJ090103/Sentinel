//! Finding the platform's cache directory.
//!
//! This is the root every path in this plugin is built from, so it is also the
//! plugin's entire safety boundary — see the crate docs. It resolves to a
//! location the *operating system* designates for disposable data, never to
//! one holding profiles.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use sentinel_plugin_api::{validate_tool_directory, PluginError};

/// Resolves the platform's cache home.
///
/// - Linux: `$XDG_CACHE_HOME`, defaulting to `~/.cache`
/// - macOS: `~/Library/Caches`
/// - Windows: unsupported, deliberately — see the crate docs
pub fn resolve() -> Result<PathBuf, PluginError> {
    resolve_from(env::var_os("XDG_CACHE_HOME"), env::var_os("HOME"))
}

/// The resolution itself, over values already read from the environment.
///
/// Split out so tests can cover each rule without mutating process state,
/// which cargo's parallel test threads would race on.
fn resolve_from(
    xdg_cache_home: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, PluginError> {
    if cfg!(windows) {
        return Err(PluginError::ToolUnavailable {
            detail: "browser caches are not supported on Windows, where the cache lives inside \
                     the profile directory alongside history and saved passwords"
                .to_owned(),
        });
    }

    let candidate = if cfg!(target_os = "macos") {
        home.map(|home| PathBuf::from(home).join("Library").join("Caches"))
    } else {
        xdg_cache_home
            .map(PathBuf::from)
            .or_else(|| home.map(|home| PathBuf::from(home).join(".cache")))
    };

    let candidate = candidate.ok_or_else(|| PluginError::ToolUnavailable {
        detail: "no cache directory could be determined for this user".to_owned(),
    })?;

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
    fn linux_prefers_xdg_cache_home() {
        let cache = tempfile::tempdir().expect("temp dir");

        assert_eq!(
            resolve_from(os(cache.path()), None).expect("resolves"),
            cache.path()
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn linux_falls_back_to_dot_cache_under_home() {
        let home = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(home.path().join(".cache")).expect("create");

        assert_eq!(
            resolve_from(None, os(home.path())).expect("resolves"),
            home.path().join(".cache")
        );
    }

    #[cfg(unix)]
    #[test]
    fn no_environment_at_all_is_not_a_crash() {
        assert!(matches!(
            resolve_from(None, None),
            Err(PluginError::ToolUnavailable { .. })
        ));
    }

    #[cfg(windows)]
    #[test]
    fn windows_is_refused_with_a_reason() {
        match resolve_from(None, None) {
            Err(PluginError::ToolUnavailable { detail }) => {
                assert!(detail.contains("Windows"), "got {detail}");
            }
            other => panic!("expected an explanatory refusal, got {other:?}"),
        }
    }
}
