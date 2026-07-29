//! Finding npm's cache directory, and refusing to believe implausible answers.
//!
//! npm's cache location is configurable — `.npmrc` at project, user, and global
//! scope, plus `npm_config_cache` in the environment — so hardcoding
//! `~/.npm` would be wrong for anyone who has moved it. The only thing that
//! resolves all of those correctly is npm itself, so this asks it.
//!
//! # Running npm, and hard rule 2
//!
//! `CLAUDE.md`'s hard rule 2 forbids shelling out to a command "constructed
//! from AI output or from unvalidated input". This is neither: the argument
//! vector is three fixed literals, passed to [`std::process::Command`], which
//! `exec`s directly without a shell. No user or AI input reaches it, and there
//! is no shell to interpret metacharacters even if it did.
//!
//! The *output* is another matter, and is where the care goes. A path that came
//! from a `.npmrc` this program did not write is untrusted input, and it is
//! about to be used to decide what gets deleted. So it is validated
//! ([`validate_cache_dir`]), and — more importantly — it is never itself a
//! deletion target. The plugin only ever proposes a fixed set of *named
//! subdirectories* of it, each of which must exist. A hostile or broken config
//! pointing the cache at `/` yields targets like `/_cacache`, which don't
//! exist, so the scan finds nothing rather than proposing to erase a disk.

use std::env;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use sentinel_plugin_api::PluginError;

/// The executable to ask. On Windows npm ships as a `.cmd` shim, which
/// `CreateProcess` won't find under the bare name.
#[cfg(windows)]
const NPM_EXECUTABLE: &str = "npm.cmd";
/// The executable to ask.
#[cfg(not(windows))]
const NPM_EXECUTABLE: &str = "npm";

/// Environment variable npm itself honours for the cache location.
///
/// Checked before running npm: it gives the same answer far more cheaply, and
/// it means the plugin still works on a machine where npm is installed through
/// a version manager that isn't on this process's `PATH`.
const CACHE_ENV_VAR: &str = "npm_config_cache";

/// Resolves npm's cache directory.
///
/// Tries, in order: the `npm_config_cache` environment variable, then
/// `npm config get cache`. Returns [`PluginError::ToolUnavailable`] if npm
/// can't be found or the answer doesn't survive [`validate_cache_dir`] —
/// "npm isn't set up here" is a normal condition on a machine without Node,
/// not a failure worth alarming anyone about.
pub fn resolve() -> Result<PathBuf, PluginError> {
    if let Some(from_env) = env::var_os(CACHE_ENV_VAR) {
        let candidate = PathBuf::from(from_env);
        return validate_cache_dir(&candidate).map(|()| candidate);
    }

    let reported = ask_npm()?;
    validate_cache_dir(&reported).map(|()| reported)
}

/// Asks npm where its cache is.
fn ask_npm() -> Result<PathBuf, PluginError> {
    // Three fixed literals. Nothing interpolated, and no shell involved.
    let output = Command::new(NPM_EXECUTABLE)
        .args(["config", "get", "cache"])
        .output()
        .map_err(|cause| match cause.kind() {
            io::ErrorKind::NotFound => PluginError::ToolUnavailable {
                detail: "npm is not installed, or not on PATH".to_owned(),
            },
            _ => PluginError::Io(cause),
        })?;

    if !output.status.success() {
        return Err(PluginError::ToolUnavailable {
            detail: format!("`npm config get cache` exited with {}", output.status),
        });
    }

    // stdout only: npm writes update notices and deprecation warnings to
    // stderr, and mixing those into a path would be nonsense.
    let reported = String::from_utf8_lossy(&output.stdout).trim().to_owned();

    // npm prints this literal when a config key has no value.
    if reported.is_empty() || reported == "undefined" || reported == "null" {
        return Err(PluginError::ToolUnavailable {
            detail: "npm did not report a cache directory".to_owned(),
        });
    }

    Ok(PathBuf::from(reported))
}

/// Rejects a cache path that Sentinel should not go anywhere near.
///
/// This is defence in depth rather than the primary protection — the plugin
/// never deletes this directory, only named children of it that must already
/// exist. But a path this implausible means the config is wrong or hostile, and
/// the right response to "npm says its cache is `/`" is to stop, not to look
/// inside and see what's there.
pub fn validate_cache_dir(candidate: &Path) -> Result<(), PluginError> {
    if !candidate.is_absolute() {
        return Err(PluginError::ToolUnavailable {
            detail: format!(
                "npm reported a relative cache path ({}), which Sentinel will not resolve",
                candidate.display()
            ),
        });
    }

    // A path with no parent is a filesystem root. Nothing legitimate puts a
    // package cache directly at `/` or `C:\`.
    if candidate.parent().is_none() {
        return Err(PluginError::ToolUnavailable {
            detail: format!(
                "npm reported a filesystem root ({}) as its cache",
                candidate.display()
            ),
        });
    }

    // `..` anywhere means the path wasn't normalised, and comparing or joining
    // it later would not mean what it looks like it means.
    if candidate
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(PluginError::ToolUnavailable {
            detail: format!(
                "npm reported a cache path containing `..` ({})",
                candidate.display()
            ),
        });
    }

    if !candidate.is_dir() {
        return Err(PluginError::ToolUnavailable {
            detail: format!(
                "npm's cache directory ({}) does not exist yet",
                candidate.display()
            ),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rejects(path: &str) -> String {
        match validate_cache_dir(Path::new(path)) {
            Err(PluginError::ToolUnavailable { detail }) => detail,
            other => panic!("expected {path} to be refused, got {other:?}"),
        }
    }

    #[test]
    fn a_relative_path_is_refused() {
        assert!(rejects("relative/cache").contains("relative"));
    }

    #[test]
    fn a_filesystem_root_is_refused() {
        let detail = rejects(if cfg!(windows) { r"C:\" } else { "/" });
        assert!(detail.contains("root"), "got {detail}");
    }

    #[test]
    fn a_path_containing_dot_dot_is_refused() {
        assert!(rejects("/home/someone/../../etc").contains(".."));
    }

    #[test]
    fn a_path_that_is_not_a_directory_is_refused() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("not-a-dir");
        std::fs::write(&file, "").expect("write file");

        let detail = match validate_cache_dir(&file) {
            Err(PluginError::ToolUnavailable { detail }) => detail,
            other => panic!("expected a file to be refused, got {other:?}"),
        };
        assert!(detail.contains("does not exist"), "got {detail}");
    }

    #[test]
    fn a_real_directory_is_accepted() {
        let dir = tempfile::tempdir().expect("temp dir");

        validate_cache_dir(dir.path()).expect("a real directory is a plausible cache");
    }
}
