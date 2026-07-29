//! Sentinel's Cargo plugin: reclaims Cargo's registry and git caches.
//!
//! Like the Bun plugin, this runs no subprocess — Cargo has no "print your
//! cache directory" command, and its layout under `CARGO_HOME` is stable and
//! documented. Targets are named children of that home, two components deep,
//! so a hostile `CARGO_HOME=/` yields `/registry/src` and friends, which don't
//! exist.
//!
//! # What is deliberately not here
//!
//! `$CARGO_HOME/bin` holds binaries installed with `cargo install`. Those are
//! not a cache — reinstalling them means recompiling, and some are not even
//! available from the same source any more. They are absent from the list
//! below, and because the list is closed, absence is sufficient: there is no
//! path by which this plugin can reach them.
//!
//! Per-project `target/` directories are also out of scope. They are usually
//! the largest thing on a Rust developer's disk, but they live wherever the
//! user's projects live, and finding them means walking the filesystem looking
//! for things that look like build output — exactly the pattern-matching this
//! project refuses to do. A future plugin could handle explicitly registered
//! project directories; guessing is not an option.

use std::path::PathBuf;

use sentinel_plugin_api::{
    directory_size, is_empty_directory, CleanFailure, CleanOutcome, CleanTarget, Plugin,
    PluginError, PluginMetadata, Recommendation, ScanResult, TargetRisk, VerifiedTarget,
};

pub mod cargo_home;

/// This plugin's stable id. Stored in cleanup history, so it is permanent.
pub const PLUGIN_ID: &str = "cargo";

/// One directory this plugin knows how to reclaim.
struct KnownCache {
    /// Path components under `CARGO_HOME`. Constants, joined onto a validated
    /// directory — nothing here is derived from user input.
    components: &'static [&'static str],
    /// What the user sees.
    label: &'static str,
    /// How much care removing it needs.
    risk: TargetRisk,
    /// Why it is safe, or what it costs.
    note: &'static str,
}

/// Every directory this plugin will ever propose.
///
/// All are `Safe` — each is rebuilt automatically by the next `cargo build` —
/// but their *costs* differ enough to be worth spelling out, because the
/// biggest one is also the cheapest to lose:
///
/// - `registry/src` is extracted crate sources, and normally the largest of
///   the three. It is re-extracted from `registry/cache` locally, with no
///   network, so clearing it is usually the best value on this list.
/// - `registry/cache` is the downloaded `.crate` archives. Clearing it means
///   re-downloading.
/// - `registry/index` is registry metadata. Clearing it means the next build
///   re-fetches the index.
///
/// Clearing `cache` *and* `src` together means a full re-download rather than
/// a local re-extract, which is why they are separate targets rather than one.
const KNOWN_CACHES: &[KnownCache] = &[
    KnownCache {
        components: &["registry", "src"],
        label: "Extracted crate sources (registry/src)",
        risk: TargetRisk::Safe,
        note: "Unpacked source of every crate you've built. Re-extracted locally from the \
               downloaded archives — no network needed, so this is usually the cheapest \
               space to reclaim.",
    },
    KnownCache {
        components: &["registry", "cache"],
        label: "Downloaded crate archives (registry/cache)",
        risk: TargetRisk::Safe,
        note: "The .crate files Cargo downloaded. Re-downloaded on demand; needs network.",
    },
    KnownCache {
        components: &["registry", "index"],
        label: "Registry index (registry/index)",
        risk: TargetRisk::Safe,
        note: "Crate metadata from the registry. Re-fetched on the next build that needs it.",
    },
    KnownCache {
        components: &["git", "checkouts"],
        label: "Git dependency checkouts (git/checkouts)",
        risk: TargetRisk::Safe,
        note: "Working copies of git dependencies. Recreated from the bare clones below.",
    },
    KnownCache {
        components: &["git", "db"],
        label: "Git dependency clones (git/db)",
        risk: TargetRisk::Safe,
        note: "Bare clones of git dependencies. Re-cloned on demand; needs network.",
    },
];

/// The Cargo cache plugin.
#[derive(Debug, Default)]
pub struct CargoPlugin {
    /// Overrides discovery. Exists for tests, which must never touch the
    /// developer's real Cargo home.
    cargo_home_override: Option<PathBuf>,
}

impl CargoPlugin {
    /// The plugin as it ships: derives `CARGO_HOME` itself.
    pub fn new() -> Self {
        Self::default()
    }

    /// A plugin pinned to a specific Cargo home, for tests.
    pub fn with_cargo_home(path: impl Into<PathBuf>) -> Self {
        Self {
            cargo_home_override: Some(path.into()),
        }
    }

    /// Cargo's home directory on this machine.
    fn cargo_home(&self) -> Result<PathBuf, PluginError> {
        match &self.cargo_home_override {
            Some(path) => sentinel_plugin_api::validate_tool_directory(path).map(|()| path.clone()),
            None => cargo_home::resolve(),
        }
    }
}

impl Plugin for CargoPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: PLUGIN_ID,
            name: "Cargo cache",
            description: "Crate sources, downloads and registry metadata Cargo keeps on disk.",
        }
    }

    fn scan(&self) -> Result<ScanResult, PluginError> {
        let home = self.cargo_home()?;

        let mut targets = Vec::new();
        for known in KNOWN_CACHES {
            // Constants joined onto a validated directory. This is the only
            // place a path is constructed here.
            let path = known
                .components
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
                format!("{} — {}", known.label, known.note),
                size,
                known.risk,
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
                "Cargo's caches are using {}",
                human_bytes(scan.total_bytes())
            ),
            detail: "Everything here is rebuilt automatically by the next build. Extracted \
                     crate sources are usually the largest and the cheapest to clear, since \
                     they are unpacked again locally without downloading anything."
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

    /// A fake Cargo home. Paths are given relative, e.g. `registry/src`.
    fn fake_home(entries: &[(&str, usize)]) -> tempfile::TempDir {
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

    #[test]
    fn metadata_is_stable() {
        assert_eq!(CargoPlugin::new().metadata().id, "cargo");
        assert_eq!(CargoPlugin::new().metadata().id, PLUGIN_ID);
    }

    #[test]
    fn scan_reports_known_caches_with_their_sizes() {
        let home = fake_home(&[("registry/src", 300), ("registry/cache", 200)]);
        let plugin = CargoPlugin::with_cargo_home(home.path());

        let scan = plugin.scan().expect("scan succeeds");

        assert_eq!(scan.targets.len(), 2);
        assert!(
            scan.total_bytes() >= 500,
            "500 bytes were written, and allocated size rounds up to whole blocks"
        );
        assert!(scan.targets.iter().all(|t| t.plugin_id() == "cargo"));
    }

    /// `cargo install` puts real binaries in `$CARGO_HOME/bin`. Reinstalling
    /// them means recompiling, and some may not be available any more — so the
    /// closed list must not reach them.
    #[test]
    fn installed_binaries_are_never_proposed() {
        let home = fake_home(&[("registry/src", 10), ("bin", 5_000)]);
        let plugin = CargoPlugin::with_cargo_home(home.path());

        let scan = plugin.scan().expect("scan succeeds");

        assert_eq!(scan.targets.len(), 1);
        assert!(
            scan.targets.iter().all(|t| !t.path().ends_with("bin")),
            "`cargo install` binaries are not a cache"
        );
        assert!(home.path().join("bin").exists());
    }

    #[test]
    fn scan_ignores_directories_it_does_not_know() {
        let home = fake_home(&[("registry/src", 10), ("my-notes", 999)]);
        let plugin = CargoPlugin::with_cargo_home(home.path());

        assert_eq!(plugin.scan().expect("scan succeeds").targets.len(), 1);
    }

    #[test]
    fn scan_never_proposes_cargo_home_itself() {
        let home = fake_home(&[("registry/src", 10)]);
        let plugin = CargoPlugin::with_cargo_home(home.path());

        assert!(plugin
            .scan()
            .expect("scan succeeds")
            .targets
            .iter()
            .all(|t| t.path() != home.path()));
    }

    #[test]
    fn git_dependency_caches_are_found_too() {
        let home = fake_home(&[("git/db", 100), ("git/checkouts", 50)]);
        let plugin = CargoPlugin::with_cargo_home(home.path());

        let scan = plugin.scan().expect("scan succeeds");

        assert_eq!(scan.targets.len(), 2);
        assert!(
            scan.total_bytes() >= 150,
            "150 bytes were written, and allocated size rounds up to whole blocks"
        );
    }

    #[test]
    fn every_cargo_cache_is_rebuilt_automatically() {
        let home = fake_home(&[
            ("registry/src", 10),
            ("registry/cache", 10),
            ("registry/index", 10),
        ]);
        let plugin = CargoPlugin::with_cargo_home(home.path());

        let scan = plugin.scan().expect("scan succeeds");

        assert!(
            scan.targets.iter().all(|t| t.risk == TargetRisk::Safe),
            "cargo rebuilds all of these without user intervention"
        );
    }

    #[test]
    fn an_empty_home_is_a_successful_scan_with_nothing_in_it() {
        let home = fake_home(&[]);
        let plugin = CargoPlugin::with_cargo_home(home.path());

        assert!(plugin.scan().expect("not an error").targets.is_empty());
    }

    #[test]
    fn a_missing_cargo_home_reads_as_tool_unavailable() {
        let plugin = CargoPlugin::with_cargo_home("/nonexistent/sentinel-test/cargo");

        assert!(matches!(
            plugin.scan(),
            Err(PluginError::ToolUnavailable { .. })
        ));
    }

    #[test]
    fn recommendations_are_explanations_not_actions() {
        let home = fake_home(&[("registry/src", 4096)]);
        let plugin = CargoPlugin::with_cargo_home(home.path());

        let recommendations = plugin.recommend().expect("recommend succeeds");

        assert_eq!(recommendations.len(), 1);
        assert!(!recommendations[0]
            .detail
            .contains(home.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn estimate_matches_the_scan() {
        let home = fake_home(&[("registry/src", 300), ("git/db", 200)]);
        let plugin = CargoPlugin::with_cargo_home(home.path());

        let scan = plugin.scan().expect("scan succeeds");
        let estimate = plugin.estimate().expect("estimate succeeds");

        assert_eq!(estimate.reclaimable_bytes, scan.total_bytes());
        assert_eq!(estimate.target_count, scan.targets.len());
    }

    #[test]
    fn scan_then_clean_removes_exactly_what_was_scanned() {
        let home = fake_home(&[("registry/src", 100), ("registry/cache", 50), ("bin", 10)]);
        let plugin = CargoPlugin::with_cargo_home(home.path());

        let scan = plugin.scan().expect("scan succeeds");
        let outcome = clean(&plugin, &scan.targets).expect("clean succeeds");

        assert_eq!(outcome.removed.len(), 2);
        assert!(
            outcome.reclaimed_bytes >= 150,
            "150 bytes were written, and allocated size rounds up to whole blocks"
        );
        assert!(!home.path().join("registry").join("src").exists());
        assert!(!home.path().join("registry").join("cache").exists());
        assert!(
            home.path().join("bin").exists(),
            "installed binaries must survive a clean"
        );
    }

    #[test]
    fn cleaning_a_subset_leaves_the_rest_alone() {
        let home = fake_home(&[("registry/src", 100), ("registry/cache", 50)]);
        let plugin = CargoPlugin::with_cargo_home(home.path());

        let scan = plugin.scan().expect("scan succeeds");
        let only_src: Vec<CleanTarget> = scan
            .targets
            .iter()
            .filter(|t| t.path().ends_with("src"))
            .cloned()
            .collect();

        clean(&plugin, &only_src).expect("clean succeeds");

        assert!(!home.path().join("registry").join("src").exists());
        assert!(
            home.path().join("registry").join("cache").exists(),
            "a target the user did not select must not be touched"
        );
    }

    #[test]
    fn a_forged_target_is_refused() {
        let home = fake_home(&[("registry/src", 10)]);
        let victim = tempfile::tempdir().expect("temp dir");
        fs::write(victim.path().join("precious"), "data").expect("write");

        let plugin = CargoPlugin::with_cargo_home(home.path());
        let forged = CleanTarget::new(PLUGIN_ID, victim.path(), "forged", 10, TargetRisk::Safe);

        let outcome = clean(&plugin, &[forged]).expect("refusal is reported, not fatal");

        assert!(outcome.removed.is_empty());
        assert_eq!(outcome.failures.len(), 1);
        assert!(victim.path().join("precious").exists());
    }

    /// Reads this machine's real Cargo home, read-only. Ignored by default.
    #[test]
    #[ignore = "reads the developer's real Cargo home; run manually"]
    fn scans_the_real_cargo_home() {
        match CargoPlugin::new().scan() {
            Ok(scan) => {
                for target in &scan.targets {
                    println!(
                        "  {:>10}  {:?}  {}",
                        human_bytes(target.size_bytes),
                        target.risk,
                        target.path().display()
                    );
                    assert!(target.path().is_absolute());
                }
                println!("total: {}", human_bytes(scan.total_bytes()));
            }
            Err(PluginError::ToolUnavailable { detail }) => {
                println!("Cargo not usable here, which is a valid outcome: {detail}");
            }
            Err(other) => panic!("unexpected scan failure: {other}"),
        }
    }

    #[test]
    fn human_bytes_reads_like_du() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1024 * 1024), "1.0 MiB");
    }
}
