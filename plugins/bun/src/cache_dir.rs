//! Deriving Bun's install-cache location.
//!
//! Unlike the npm and pip plugins, this asks no subprocess. `bun pm cache`
//! would be the obvious equivalent of `npm config get cache`, but it is a
//! *project* command: run outside a directory containing a `package.json` it
//! exits with an error rather than printing a path. Sentinel has no project to
//! run it in, so the layout is derived instead.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use sentinel_plugin_api::{validate_tool_directory, PluginError};

/// Where Bun installs itself, and therefore where its cache lives.
const BUN_HOME_ENV_VAR: &str = "BUN_INSTALL";

/// The env var Bun uses to relocate its cache. Deliberately not honoured — see
/// [`resolve`].
const CACHE_OVERRIDE_ENV_VAR: &str = "BUN_INSTALL_CACHE_DIR";

/// Path components appended to Bun's home to reach the cache. Constants, so a
/// hostile `BUN_INSTALL` still can't name an arbitrary directory.
const CACHE_COMPONENTS: [&str; 2] = ["install", "cache"];

/// Resolves Bun's install cache.
///
/// `$BUN_INSTALL/install/cache`, falling back to `~/.bun/install/cache`.
///
/// # Why `BUN_INSTALL_CACHE_DIR` is ignored
///
/// Bun lets you point its cache anywhere with that variable, and honouring it
/// would be a one-line change. It is refused on purpose.
///
/// For npm and pip, a hostile cache path is harmless: those plugins only ever
/// propose *named children* of it, so a cache configured as `$HOME` yields
/// candidates like `$HOME/_cacache` that don't exist. Bun's cache directory is
/// itself the target, so honouring an arbitrary configured path would mean
/// proposing to delete a directory named entirely by configuration — which is
/// precisely what the closed-list rule exists to prevent. A stray
/// `BUN_INSTALL_CACHE_DIR=$HOME` would put a user's home directory on the
/// review screen.
///
/// So it is reported rather than silently ignored: a user who has relocated
/// their cache is told Sentinel isn't managing it, instead of quietly seeing
/// nothing. Supporting it safely needs a way to validate an arbitrary
/// configured directory, which is a real design question rather than a line of
/// code.
pub fn resolve() -> Result<PathBuf, PluginError> {
    resolve_from(
        env::var_os(CACHE_OVERRIDE_ENV_VAR),
        env::var_os(BUN_HOME_ENV_VAR),
        env::var_os("HOME"),
    )
}

/// The resolution itself, over values already read from the environment.
///
/// Split out so the tests can cover every combination without mutating process
/// state. Tests that `set_var` would race each other — cargo runs them in
/// parallel threads of one process — and a racing test is worse than no test,
/// because it fails on someone else's unrelated change.
fn resolve_from(
    cache_override: Option<OsString>,
    bun_home: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, PluginError> {
    if cache_override.is_some() {
        return Err(PluginError::ToolUnavailable {
            detail: format!(
                "{CACHE_OVERRIDE_ENV_VAR} is set, so Bun's cache has been moved. Sentinel \
                 only manages the default location and will not act on a directory named \
                 entirely by configuration."
            ),
        });
    }

    let root = bun_home_from(bun_home, home)?;
    // Two constants joined onto a validated directory.
    let cache = CACHE_COMPONENTS
        .iter()
        .fold(root, |path, component| path.join(component));

    validate_tool_directory(&cache).map(|()| cache)
}

/// Bun's home directory: `$BUN_INSTALL`, else `~/.bun`.
fn bun_home_from(
    bun_home: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, PluginError> {
    if let Some(configured) = bun_home {
        let path = PathBuf::from(configured);
        // Validated before anything is joined onto it, so a nonsense value is
        // rejected here rather than turning into a nonsense cache path.
        validate_tool_directory(&path)?;
        return Ok(path);
    }

    let home = home
        .map(PathBuf::from)
        .ok_or_else(|| PluginError::ToolUnavailable {
            detail: "no home directory, so Bun's default cache location can't be derived"
                .to_owned(),
        })?;

    Ok(home.join(".bun"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(path: &std::path::Path) -> Option<OsString> {
        Some(path.as_os_str().to_owned())
    }

    #[test]
    fn a_relocated_cache_is_reported_rather_than_silently_ignored() {
        let dir = tempfile::tempdir().expect("temp dir");

        let error = resolve_from(os(dir.path()), None, os(dir.path()))
            .expect_err("a relocated cache must not be acted on");

        match error {
            PluginError::ToolUnavailable { detail } => {
                assert!(detail.contains(CACHE_OVERRIDE_ENV_VAR), "got {detail}");
            }
            other => panic!("expected ToolUnavailable, got {other:?}"),
        }
    }

    /// The safety property: the cache path is always two constants joined onto
    /// Bun's home, so configuration can't name an arbitrary directory.
    #[test]
    fn the_cache_path_is_always_under_bun_home() {
        let home = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(home.path().join("install").join("cache")).expect("create cache");

        let resolved = resolve_from(None, os(home.path()), None).expect("a real cache resolves");

        assert_eq!(resolved, home.path().join("install").join("cache"));
    }

    #[test]
    fn the_default_location_is_under_the_home_directory() {
        let home = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(home.path().join(".bun").join("install").join("cache"))
            .expect("create cache");

        let resolved = resolve_from(None, None, os(home.path())).expect("resolves");

        assert_eq!(
            resolved,
            home.path().join(".bun").join("install").join("cache")
        );
    }

    #[test]
    fn a_bun_home_without_a_cache_reads_as_tool_unavailable() {
        let home = tempfile::tempdir().expect("temp dir");

        assert!(
            matches!(
                resolve_from(None, os(home.path()), None),
                Err(PluginError::ToolUnavailable { .. })
            ),
            "a Bun that has never installed anything has no cache yet"
        );
    }

    #[test]
    fn a_nonsense_bun_home_is_refused_before_anything_is_joined_onto_it() {
        assert!(matches!(
            resolve_from(None, Some(OsString::from("relative/bun")), None),
            Err(PluginError::ToolUnavailable { .. })
        ));
    }

    #[test]
    fn no_home_directory_at_all_is_not_a_crash() {
        assert!(matches!(
            resolve_from(None, None, None),
            Err(PluginError::ToolUnavailable { .. })
        ));
    }
}
