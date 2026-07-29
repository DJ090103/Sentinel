//! Sentinel's browser plugin: reclaims browser caches, and only caches.
//!
//! # The safety property, and why this plugin is simpler than the editor one
//!
//! A browser profile holds history, cookies, saved passwords and bookmarks —
//! the most sensitive data on most machines. This plugin is built so that it
//! cannot reach any of it, and the reason is structural rather than careful:
//!
//! On Linux and macOS the operating system's conventions put browser caches in
//! a **different directory tree** from browser profiles:
//!
//! | | Cache (this plugin) | Profile (never touched) |
//! | --- | --- | --- |
//! | Linux | `~/.cache/google-chrome` | `~/.config/google-chrome` |
//! | macOS | `~/Library/Caches/Google/Chrome` | `~/Library/Application Support/Google/Chrome` |
//!
//! Every path this plugin builds is rooted at the *cache* location. It never
//! constructs a path under the profile location, so `History`, `Cookies` and
//! `Login Data` are not merely excluded from a list — they are in a tree this
//! code has no expression for.
//!
//! Verified on the development machine: `~/.config/google-chrome/Default`
//! contains `History`, `Cookies`, `Login Data` and `Bookmarks`, while
//! `~/.cache/google-chrome/Default` contains only `Cache` and `Code Cache`.
//!
//! # Windows is deliberately unsupported
//!
//! Windows has no such split — Chromium keeps its cache *inside* the profile
//! directory (`…\User Data\Default\Cache`, a sibling of `History` and
//! `Login Data`). Supporting it would mean naming paths inside the profile
//! tree, which throws away the structural guarantee above and replaces it with
//! a promise not to make a mistake. Until that is designed properly, Windows
//! reports "not supported" rather than getting a weaker version of this.

use std::path::PathBuf;

use sentinel_plugin_api::{
    directory_size, is_empty_directory, CleanFailure, CleanOutcome, CleanTarget, Plugin,
    PluginError, PluginMetadata, Recommendation, ScanResult, TargetRisk, VerifiedTarget,
};

pub mod cache_home;

/// This plugin's stable id. Stored in cleanup history, so it is permanent.
pub const PLUGIN_ID: &str = "browser";

/// One browser this plugin knows about.
struct Browser {
    /// What the user sees.
    name: &'static str,
    /// Path components under the platform's cache home. Constants.
    linux: &'static [&'static str],
    /// Path components under `~/Library/Caches`. Constants.
    macos: &'static [&'static str],
}

impl Browser {
    /// The components to use on this platform, or `None` where unsupported.
    fn components(&self) -> Option<&'static [&'static str]> {
        if cfg!(target_os = "macos") {
            Some(self.macos)
        } else if cfg!(unix) {
            Some(self.linux)
        } else {
            // See the crate docs: Windows keeps the cache inside the profile.
            None
        }
    }
}

/// Every browser this plugin will ever look at.
///
/// Each entry names a *cache* directory. Chromium browsers keep one
/// subdirectory per profile ("Default", "Profile 2", …) beneath it, and those
/// names vary per user — so the browser's cache directory as a whole is the
/// target rather than enumerating profiles. That is safe here in a way it
/// would not be elsewhere: everything beneath a cache home is, by the platform's
/// own convention, disposable.
const BROWSERS: &[Browser] = &[
    Browser {
        name: "Google Chrome",
        linux: &["google-chrome"],
        macos: &["Google", "Chrome"],
    },
    Browser {
        name: "Chromium",
        linux: &["chromium"],
        macos: &["Chromium"],
    },
    Browser {
        name: "Brave",
        linux: &["BraveSoftware", "Brave-Browser"],
        macos: &["BraveSoftware", "Brave-Browser"],
    },
    Browser {
        name: "Microsoft Edge",
        linux: &["microsoft-edge"],
        macos: &["Microsoft Edge"],
    },
    Browser {
        name: "Vivaldi",
        linux: &["vivaldi"],
        macos: &["Vivaldi"],
    },
    Browser {
        name: "Firefox",
        linux: &["mozilla", "firefox"],
        macos: &["Firefox"],
    },
];

/// The browser cache plugin.
#[derive(Debug, Default)]
pub struct BrowserPlugin {
    /// Overrides the cache home. Exists for tests, which must never touch the
    /// developer's real browser data.
    cache_home_override: Option<PathBuf>,
}

impl BrowserPlugin {
    /// The plugin as it ships: finds the platform's cache home itself.
    pub fn new() -> Self {
        Self::default()
    }

    /// A plugin pinned to a specific cache home, for tests.
    pub fn with_cache_home(path: impl Into<PathBuf>) -> Self {
        Self {
            cache_home_override: Some(path.into()),
        }
    }

    /// The platform's cache directory.
    fn cache_home(&self) -> Result<PathBuf, PluginError> {
        match &self.cache_home_override {
            Some(path) => sentinel_plugin_api::validate_tool_directory(path).map(|()| path.clone()),
            None => cache_home::resolve(),
        }
    }
}

impl Plugin for BrowserPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: PLUGIN_ID,
            name: "Browser caches",
            description: "Cached web content only — never history, cookies, passwords or \
                          bookmarks.",
        }
    }

    fn scan(&self) -> Result<ScanResult, PluginError> {
        let home = self.cache_home()?;

        let mut targets = Vec::new();
        for browser in BROWSERS {
            let Some(components) = browser.components() else {
                continue;
            };

            // Constants joined onto the platform's cache directory. This is the
            // only place a path is constructed, and nothing in it is variable.
            let path = components
                .iter()
                .fold(home.clone(), |path, component| path.join(component));

            // `symlink_metadata` so a symlink is seen as a symlink rather than
            // as whatever it points at.
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if !metadata.is_dir() {
                continue;
            }

            if is_empty_directory(&path) {
                // Nothing in it, so nothing to reclaim — which is a different
                // question from measuring zero. See `is_empty_directory`.
                continue;
            }
            let size = directory_size(&path)?;

            targets.push(CleanTarget::new(
                PLUGIN_ID,
                path,
                format!(
                    "{} cache — Cached pages, images and scripts. Re-downloaded as you browse; \
                     sites you visit next will load a little slower. Your history, cookies, \
                     passwords and bookmarks are stored separately and are not touched.",
                    browser.name
                ),
                size,
                // A browser cache is the archetype of regenerable: every byte
                // came off the network and goes back on demand.
                TargetRisk::Safe,
            ));
        }

        Ok(ScanResult { targets })
    }

    fn recommend(&self) -> Result<Vec<Recommendation>, PluginError> {
        let scan = self.scan()?;
        if scan.targets.is_empty() {
            return Ok(Vec::new());
        }

        Ok(vec![Recommendation {
            title: format!(
                "Browser caches are using {}",
                human_bytes(scan.total_bytes())
            ),
            detail: "Pages, images and scripts your browsers saved to load sites faster. All of \
                     it is re-downloaded as you browse. Signed-in sessions, history and saved \
                     passwords live elsewhere and are unaffected. Closing the browser first is \
                     tidier, though not required."
                .to_owned(),
            reclaimable_bytes: scan.total_bytes(),
            risk: TargetRisk::Safe,
        }])
    }

    fn remove_verified(
        &self,
        verified: &[VerifiedTarget<'_>],
    ) -> Result<CleanOutcome, PluginError> {
        let mut outcome = CleanOutcome::default();

        for target in verified {
            // Measured before removal, so this reports what was actually freed
            // rather than the scan-time estimate.
            let freed = directory_size(target.path()).unwrap_or(0);

            match std::fs::remove_dir_all(target.path()) {
                Ok(()) => {
                    outcome.removed.push(target.label().to_owned());
                    outcome.reclaimed_bytes = outcome.reclaimed_bytes.saturating_add(freed);
                }
                Err(cause) => outcome.failures.push(CleanFailure {
                    label: target.label().to_owned(),
                    reason: cause.to_string(),
                }),
            }
        }

        Ok(outcome)
    }
}

/// Formats bytes in binary units, so the figure matches `du`.
fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_plugin_api::clean;
    use std::fs;

    /// A fake cache home. Paths are relative, e.g. `google-chrome/Default`.
    fn fake_cache_home(entries: &[(&str, usize)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        for (relative, size) in entries {
            let path = relative
                .split('/')
                .fold(dir.path().to_path_buf(), |path, part| path.join(part));
            fs::create_dir_all(&path).expect("create directory");
            fs::write(path.join("blob"), vec![b'x'; *size]).expect("write blob");
        }
        dir
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn scan_finds_each_installed_browser() {
        let home = fake_cache_home(&[
            ("google-chrome/Default/Cache", 300),
            ("BraveSoftware/Brave-Browser/Default/Cache", 200),
        ]);
        let plugin = BrowserPlugin::with_cache_home(home.path());

        let scan = plugin.scan().expect("scan succeeds");

        assert_eq!(scan.targets.len(), 2);
        assert!(
            scan.total_bytes() >= 500,
            "500 bytes were written, and allocated size rounds up to whole blocks"
        );
        assert!(scan.targets.iter().all(|t| t.plugin_id() == "browser"));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn a_browser_that_is_not_installed_is_simply_absent() {
        let home = fake_cache_home(&[("google-chrome/Default/Cache", 10)]);
        let plugin = BrowserPlugin::with_cache_home(home.path());

        let scan = plugin.scan().expect("scan succeeds");

        assert_eq!(scan.targets.len(), 1);
        assert!(scan.targets.iter().all(|t| !t.path().ends_with("vivaldi")));
    }

    /// The structural guarantee: every path this plugin builds is rooted at the
    /// cache home, so nothing under the profile tree is expressible.
    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn no_target_ever_leaves_the_cache_directory() {
        let home = fake_cache_home(&[
            ("google-chrome/Default/Cache", 10),
            ("mozilla/firefox/abc.default/cache2", 10),
        ]);
        let plugin = BrowserPlugin::with_cache_home(home.path());

        let scan = plugin.scan().expect("scan succeeds");

        assert!(!scan.targets.is_empty());
        for target in &scan.targets {
            assert!(
                target.path().starts_with(home.path()),
                "{} escaped the cache home",
                target.path().display()
            );
        }
    }

    /// Profile data lives in a sibling tree. Even when it sits right next to
    /// the cache home, nothing in it can be reached.
    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn profile_data_beside_the_cache_home_is_untouchable() {
        let root = tempfile::tempdir().expect("temp dir");
        let cache_home = root.path().join(".cache");
        let config_home = root.path().join(".config");

        fs::create_dir_all(
            cache_home
                .join("google-chrome")
                .join("Default")
                .join("Cache"),
        )
        .expect("create cache");
        fs::write(
            cache_home
                .join("google-chrome")
                .join("Default")
                .join("Cache")
                .join("blob"),
            vec![b'x'; 100],
        )
        .expect("write");

        let profile = config_home.join("google-chrome").join("Default");
        fs::create_dir_all(&profile).expect("create profile");
        for sensitive in ["History", "Cookies", "Login Data", "Bookmarks"] {
            fs::write(profile.join(sensitive), "sensitive").expect("write");
        }

        let plugin = BrowserPlugin::with_cache_home(&cache_home);
        let scan = plugin.scan().expect("scan succeeds");
        clean(&plugin, &scan.targets).expect("clean succeeds");

        for sensitive in ["History", "Cookies", "Login Data", "Bookmarks"] {
            assert!(
                profile.join(sensitive).exists(),
                "`{sensitive}` must be untouchable"
            );
        }
    }

    #[test]
    fn a_missing_cache_home_reads_as_tool_unavailable() {
        let plugin = BrowserPlugin::with_cache_home("/nonexistent/sentinel-test/cache");

        assert!(matches!(
            plugin.scan(),
            Err(PluginError::ToolUnavailable { .. })
        ));
    }

    #[test]
    fn no_browsers_installed_is_a_successful_empty_scan() {
        let home = fake_cache_home(&[]);
        let plugin = BrowserPlugin::with_cache_home(home.path());

        assert!(plugin.scan().expect("not an error").targets.is_empty());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn recommendations_are_explanations_not_actions() {
        let home = fake_cache_home(&[("google-chrome/Default/Cache", 4096)]);
        let plugin = BrowserPlugin::with_cache_home(home.path());

        let recommendations = plugin.recommend().expect("recommend succeeds");

        assert_eq!(recommendations.len(), 1);
        assert!(!recommendations[0]
            .detail
            .contains(home.path().to_string_lossy().as_ref()));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn scan_then_clean_removes_exactly_what_was_scanned() {
        let home = fake_cache_home(&[
            ("google-chrome/Default/Cache", 100),
            ("something-else/data", 50),
        ]);
        let plugin = BrowserPlugin::with_cache_home(home.path());

        let scan = plugin.scan().expect("scan succeeds");
        let outcome = clean(&plugin, &scan.targets).expect("clean succeeds");

        assert_eq!(outcome.removed.len(), 1);
        assert!(
            outcome.reclaimed_bytes >= 100,
            "100 bytes were written, and allocated size rounds up to whole blocks"
        );
        assert!(!home.path().join("google-chrome").exists());
        assert!(
            home.path().join("something-else").exists(),
            "a directory this plugin doesn't know must survive"
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn a_forged_target_is_refused() {
        let home = fake_cache_home(&[("google-chrome/Default/Cache", 10)]);
        let victim = tempfile::tempdir().expect("temp dir");
        fs::write(victim.path().join("History"), "sensitive").expect("write");

        let plugin = BrowserPlugin::with_cache_home(home.path());
        let forged = CleanTarget::new(PLUGIN_ID, victim.path(), "forged", 10, TargetRisk::Safe);

        let outcome = clean(&plugin, &[forged]).expect("refusal is reported, not fatal");

        assert!(outcome.removed.is_empty());
        assert!(victim.path().join("History").exists());
    }

    /// Reads this machine's real browser caches, read-only. Ignored by default.
    #[test]
    #[ignore = "reads the developer's real browser caches; run manually"]
    fn scans_the_real_browser_caches() {
        match BrowserPlugin::new().scan() {
            Ok(scan) => {
                for target in &scan.targets {
                    println!(
                        "  {:>10}  {}",
                        human_bytes(target.size_bytes),
                        target.path().display()
                    );
                    assert!(target.path().is_absolute());
                }
                println!("total: {}", human_bytes(scan.total_bytes()));
            }
            Err(PluginError::ToolUnavailable { detail }) => {
                println!("no browser caches here: {detail}");
            }
            Err(other) => panic!("unexpected scan failure: {other}"),
        }
    }
}
