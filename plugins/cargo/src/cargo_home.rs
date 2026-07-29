//! Deriving Cargo's home directory.
//!
//! No subprocess: Cargo has no command that prints its cache location, and its
//! layout under `CARGO_HOME` is stable and documented. Deriving is both simpler
//! and one less thing touching `CLAUDE.md` hard rule 2.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use sentinel_plugin_api::{validate_tool_directory, PluginError};

/// The variable Cargo itself honours.
const CARGO_HOME_ENV_VAR: &str = "CARGO_HOME";

/// Cargo's default home, relative to the user's home directory.
const DEFAULT_DIRECTORY: &str = ".cargo";

/// Resolves Cargo's home directory.
///
/// `$CARGO_HOME`, falling back to `~/.cargo`. Returns
/// [`PluginError::ToolUnavailable`] when neither exists — a machine without
/// Rust is a normal condition, not a failure.
///
/// Unlike Bun's `BUN_INSTALL_CACHE_DIR`, honouring `CARGO_HOME` is safe: it
/// names a *home*, and the plugin only proposes named children of it
/// (`registry/src` and friends). A hostile `CARGO_HOME=$HOME` yields
/// candidates like `$HOME/registry/src`, which won't exist.
pub fn resolve() -> Result<PathBuf, PluginError> {
    resolve_from(env::var_os(CARGO_HOME_ENV_VAR), env::var_os("HOME"))
}

/// The resolution itself, over values already read from the environment.
///
/// Split out so tests can cover every case without mutating process state,
/// which cargo's parallel test threads would race on.
fn resolve_from(
    cargo_home: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, PluginError> {
    let candidate = match cargo_home {
        Some(configured) => PathBuf::from(configured),
        None => home
            .map(PathBuf::from)
            .ok_or_else(|| PluginError::ToolUnavailable {
                detail: "no home directory, so Cargo's default location can't be derived"
                    .to_owned(),
            })?
            .join(DEFAULT_DIRECTORY),
    };

    validate_tool_directory(&candidate).map(|()| candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(path: &std::path::Path) -> Option<OsString> {
        Some(path.as_os_str().to_owned())
    }

    #[test]
    fn the_configured_home_is_used_when_it_exists() {
        let dir = tempfile::tempdir().expect("temp dir");

        assert_eq!(
            resolve_from(os(dir.path()), None).expect("resolves"),
            dir.path()
        );
    }

    #[test]
    fn the_default_is_dot_cargo_under_the_home_directory() {
        let home = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(home.path().join(DEFAULT_DIRECTORY)).expect("create .cargo");

        assert_eq!(
            resolve_from(None, os(home.path())).expect("resolves"),
            home.path().join(DEFAULT_DIRECTORY)
        );
    }

    #[test]
    fn a_machine_without_rust_reads_as_tool_unavailable() {
        let home = tempfile::tempdir().expect("temp dir");

        assert!(matches!(
            resolve_from(None, os(home.path())),
            Err(PluginError::ToolUnavailable { .. })
        ));
    }

    #[test]
    fn a_relative_cargo_home_is_refused() {
        assert!(matches!(
            resolve_from(Some(OsString::from("relative/cargo")), None),
            Err(PluginError::ToolUnavailable { .. })
        ));
    }

    #[test]
    fn no_home_directory_at_all_is_not_a_crash() {
        assert!(matches!(
            resolve_from(None, None),
            Err(PluginError::ToolUnavailable { .. })
        ));
    }
}
