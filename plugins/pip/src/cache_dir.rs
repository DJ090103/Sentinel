//! Finding pip's cache directory.
//!
//! Same shape as the npm plugin's equivalent, and for the same reasons: pip's
//! cache location is configurable (`PIP_CACHE_DIR`, `pip.conf` at several
//! scopes, `--cache-dir`), so hardcoding `~/.cache/pip` would be wrong for
//! anyone who has moved it. The only thing that resolves all of those correctly
//! is pip.
//!
//! # Running pip, and hard rule 2
//!
//! `CLAUDE.md`'s hard rule 2 forbids shelling out to a command "constructed
//! from AI output or from unvalidated input". This is neither: two fixed
//! literals passed to [`std::process::Command`], which `exec`s directly with no
//! shell involved.
//!
//! The *output* is untrusted — it comes from a config file Sentinel didn't
//! write — so it goes through
//! [`sentinel_plugin_api::validate_tool_directory`], and is never itself a
//! deletion target. The plugin only proposes named children of it.

use std::env;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::Command;

use sentinel_plugin_api::{validate_tool_directory, PluginError};

/// Executables to try, in order.
///
/// `pip3` as well as `pip` because plenty of distributions ship only the
/// former, and on Windows the shim carries an extension `CreateProcess` won't
/// infer.
#[cfg(windows)]
const PIP_EXECUTABLES: &[&str] = &["pip.exe", "pip3.exe"];
/// Executables to try, in order.
#[cfg(not(windows))]
const PIP_EXECUTABLES: &[&str] = &["pip", "pip3"];

/// Environment variable pip itself honours for the cache location.
///
/// Checked before running pip: it gives the same answer far more cheaply, and
/// it still works on a machine where pip lives in a virtualenv that isn't on
/// this process's `PATH`.
const CACHE_ENV_VAR: &str = "PIP_CACHE_DIR";

/// Resolves pip's cache directory.
///
/// Tries `PIP_CACHE_DIR`, then `pip cache dir`. Returns
/// [`PluginError::ToolUnavailable`] if pip can't be found or the answer doesn't
/// validate — "pip isn't set up here" is a normal condition on a machine
/// without Python, not a failure worth alarming anyone about.
pub fn resolve() -> Result<PathBuf, PluginError> {
    resolve_from(env::var_os(CACHE_ENV_VAR))
}

/// The resolution itself, over a value already read from the environment.
///
/// Split out so the tests can cover the configured case without mutating
/// process state. Tests that `set_var` race each other — cargo runs them in
/// parallel threads of one process — and a racing test is worse than no test,
/// because it fails on someone else's unrelated change.
fn resolve_from(configured: Option<OsString>) -> Result<PathBuf, PluginError> {
    if let Some(from_env) = configured {
        let candidate = PathBuf::from(from_env);
        return validate_tool_directory(&candidate).map(|()| candidate);
    }

    let reported = ask_pip()?;
    validate_tool_directory(&reported).map(|()| reported)
}

/// Asks pip where its cache is.
fn ask_pip() -> Result<PathBuf, PluginError> {
    let mut last_missing = None;

    for executable in PIP_EXECUTABLES {
        // Two fixed literals. Nothing interpolated, and no shell involved.
        let output = match Command::new(executable).args(["cache", "dir"]).output() {
            Ok(output) => output,
            // Try the next name rather than giving up: `pip` missing while
            // `pip3` exists is the common case on Debian-family systems.
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                last_missing = Some(*executable);
                continue;
            }
            Err(cause) => return Err(PluginError::Io(cause)),
        };

        if !output.status.success() {
            // `pip cache dir` fails when the cache is disabled (`--no-cache-dir`
            // in a config). That's a legitimate configuration, not a fault.
            return Err(PluginError::ToolUnavailable {
                detail: format!("`{executable} cache dir` exited with {}", output.status),
            });
        }

        // stdout only: pip writes upgrade notices and deprecation warnings to
        // stderr, and mixing those into a path would be nonsense.
        let reported = String::from_utf8_lossy(&output.stdout).trim().to_owned();

        if reported.is_empty() {
            return Err(PluginError::ToolUnavailable {
                detail: format!("`{executable} cache dir` reported no directory"),
            });
        }

        return Ok(PathBuf::from(reported));
    }

    Err(PluginError::ToolUnavailable {
        detail: match last_missing {
            Some(_) => "pip is not installed, or not on PATH".to_owned(),
            None => "pip could not be run".to_owned(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The environment variable short-circuits running pip entirely, which is
    /// what makes this plugin work inside a virtualenv Sentinel can't see.
    #[test]
    fn the_environment_variable_is_honoured_when_it_points_somewhere_real() {
        let dir = tempfile::tempdir().expect("temp dir");

        let resolved = resolve_from(Some(dir.path().as_os_str().to_owned()));

        assert_eq!(resolved.expect("a real directory resolves"), dir.path());
    }

    #[test]
    fn a_nonexistent_environment_path_reads_as_tool_unavailable() {
        let resolved = resolve_from(Some(OsString::from("/nonexistent/sentinel-test/pip")));

        assert!(
            matches!(resolved, Err(PluginError::ToolUnavailable { .. })),
            "got {resolved:?}"
        );
    }

    #[test]
    fn a_relative_environment_path_is_refused() {
        assert!(matches!(
            resolve_from(Some(OsString::from("relative/pip-cache"))),
            Err(PluginError::ToolUnavailable { .. })
        ));
    }
}
