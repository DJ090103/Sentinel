//! Sentinel's pip plugin: reclaims the caches pip keeps on disk.
//!
//! Follows `docs/PLUGIN_SDK.md` and the npm plugin it documents. The shape is
//! deliberately the same — a closed list of named subdirectories of a validated
//! cache root — so that reading one plugin teaches you all of them.
//!
//! # The one judgement that differs from npm
//!
//! pip's `wheels` directory holds wheels pip **built locally** from source
//! distributions, not wheels it downloaded. Rebuilding them can mean compiling
//! C extensions — for something like `numpy` or `cryptography` that is minutes
//! of CPU, not a re-download. So it is [`TargetRisk::Review`], while pip's
//! actual download cache is `Safe`. Same rule as npm's `_logs`: the level
//! describes the consequence of being wrong, not the size of the directory.

use std::path::PathBuf;

use sentinel_plugin_api::{
    directory_size, is_empty_directory, validate_tool_directory, CleanFailure, CleanOutcome,
    CleanTarget, Plugin, PluginError, PluginMetadata, Recommendation, ScanResult, TargetRisk,
    VerifiedTarget,
};

pub mod cache_dir;

/// This plugin's stable id. Stored in cleanup history, so it is permanent.
pub const PLUGIN_ID: &str = "pip";

/// One directory this plugin knows how to reclaim.
struct KnownCache {
    /// Directory name directly under pip's cache root.
    name: &'static str,
    /// What the user sees.
    label: &'static str,
    /// How much care removing it needs.
    risk: TargetRisk,
    /// Why it is safe, or what is lost.
    note: &'static str,
}

/// Every directory this plugin will ever propose.
///
/// `http-v2` is pip's HTTP response cache; `http` is what older pip called the
/// same thing, and both are listed because a machine that has upgraded pip may
/// still have the old one sitting there. `selfcheck` is a handful of timestamps
/// recording when pip last looked for a newer pip.
///
/// `wheels` is the one to think about — see the module docs.
const KNOWN_CACHES: &[KnownCache] = &[
    KnownCache {
        name: "http-v2",
        label: "Download cache (http-v2)",
        risk: TargetRisk::Safe,
        note: "Downloaded package archives. pip re-fetches what it needs; the next \
               install is slower.",
    },
    KnownCache {
        name: "http",
        label: "Download cache (http, older pip)",
        risk: TargetRisk::Safe,
        note: "The same cache under the name older pip versions used. Re-downloaded \
               on demand.",
    },
    KnownCache {
        name: "wheels",
        label: "Locally built wheels (wheels)",
        risk: TargetRisk::Review,
        note: "Wheels pip built on this machine from source. Rebuilding them compiles \
               again — minutes of CPU for packages with C extensions, not just a \
               re-download.",
    },
    KnownCache {
        name: "selfcheck",
        label: "Version-check timestamps (selfcheck)",
        risk: TargetRisk::Safe,
        note: "Records of when pip last checked for a newer pip. Recreated on the next \
               run.",
    },
];

/// The pip cache plugin.
///
/// Holds no state between calls; the cache location is resolved on every scan,
/// so a virtualenv or a changed `PIP_CACHE_DIR` takes effect without a restart.
#[derive(Debug, Default)]
pub struct PipPlugin {
    /// Overrides discovery. Exists for tests, which must never touch the
    /// developer's real pip cache.
    cache_root_override: Option<PathBuf>,
}

impl PipPlugin {
    /// The plugin as it ships: discovers pip's cache itself.
    pub fn new() -> Self {
        Self::default()
    }

    /// A plugin pinned to a specific cache directory, for tests.
    pub fn with_cache_root(root: impl Into<PathBuf>) -> Self {
        Self {
            cache_root_override: Some(root.into()),
        }
    }

    /// Where pip keeps its cache on this machine.
    fn cache_root(&self) -> Result<PathBuf, PluginError> {
        match &self.cache_root_override {
            Some(root) => validate_tool_directory(root).map(|()| root.clone()),
            None => cache_dir::resolve(),
        }
    }
}

impl Plugin for PipPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: PLUGIN_ID,
            name: "pip cache",
            description: "Package downloads and locally built wheels that pip keeps on disk.",
        }
    }

    fn scan(&self) -> Result<ScanResult, PluginError> {
        let root = self.cache_root()?;

        let mut targets = Vec::new();
        for known in KNOWN_CACHES {
            // A constant joined onto a validated root. This is the only place a
            // path is constructed, and nothing in it is variable.
            let path = root.join(known.name);

            // `symlink_metadata` so a symlink is seen as a symlink rather than
            // as whatever it points at.
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if !metadata.is_dir() {
                continue;
            }

            if is_empty_directory(&path) {
                // Nothing in it, so nothing to reclaim. A *purged* cache is not
                // empty — `http-v2` keeps its directory tree — and is still
                // offered, which is the point of measuring blocks.
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

        let mut recommendations = Vec::new();

        // One per risk level, not one per directory: the user is deciding "do I
        // clear downloads?" and "do I clear things I'd have to rebuild?", which
        // are genuinely different questions.
        for risk in [TargetRisk::Safe, TargetRisk::Review] {
            let bytes = scan
                .targets
                .iter()
                .filter(|target| target.risk == risk)
                .fold(0u64, |sum, target| sum.saturating_add(target.size_bytes));

            if bytes == 0 {
                continue;
            }

            recommendations.push(match risk {
                TargetRisk::Safe => Recommendation {
                    title: format!("pip downloads are using {}", human_bytes(bytes)),
                    detail: "Package archives pip can fetch again. Clearing them frees the \
                             space now and costs a slower next install."
                        .to_owned(),
                    reclaimable_bytes: bytes,
                    risk,
                },
                _ => Recommendation {
                    title: format!("Locally built wheels are using {}", human_bytes(bytes)),
                    detail: "Wheels compiled on this machine. Removing them frees the space, \
                             but the next install of those packages has to build them again — \
                             which can take minutes for packages with C extensions."
                        .to_owned(),
                    reclaimable_bytes: bytes,
                    risk,
                },
            });
        }

        Ok(recommendations)
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
                // One failure does not abandon the rest.
                Err(cause) => outcome.failures.push(CleanFailure {
                    label: target.label().to_owned(),
                    reason: cause.to_string(),
                }),
            }
        }

        Ok(outcome)
    }
}

/// Formats bytes for prose in a recommendation, in binary units so the figure
/// matches what `du` would say.
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

    /// Builds a fake pip cache with the given directories, each holding one
    /// file of the given size.
    fn fake_cache(entries: &[(&str, usize)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");

        for (name, size) in entries {
            let path = dir.path().join(name);
            fs::create_dir_all(&path).expect("create cache subdirectory");
            fs::write(path.join("blob"), vec![b'x'; *size]).expect("write blob");
        }

        dir
    }

    #[test]
    fn metadata_is_stable() {
        assert_eq!(PipPlugin::new().metadata().id, "pip");
        assert_eq!(PipPlugin::new().metadata().id, PLUGIN_ID);
    }

    #[test]
    fn scan_reports_known_caches_with_their_sizes() {
        let cache = fake_cache(&[("http-v2", 400), ("wheels", 100)]);
        let plugin = PipPlugin::with_cache_root(cache.path());

        let scan = plugin.scan().expect("scan succeeds");

        assert_eq!(scan.targets.len(), 2);
        assert!(
            scan.total_bytes() >= 500,
            "500 bytes were written, and allocated size rounds up to whole blocks"
        );
        assert!(scan.targets.iter().all(|t| t.plugin_id() == "pip"));
    }

    /// The closed list is the safety property: a virtualenv or anything else a
    /// user parked in the cache directory is invisible to this plugin.
    #[test]
    fn scan_ignores_directories_it_does_not_know() {
        let cache = fake_cache(&[("http-v2", 10), ("my-notes", 999)]);
        let plugin = PipPlugin::with_cache_root(cache.path());

        let scan = plugin.scan().expect("scan succeeds");

        assert_eq!(scan.targets.len(), 1);
        assert!(scan.targets[0].path().ends_with("http-v2"));
    }

    #[test]
    fn scan_never_proposes_the_cache_root_itself() {
        let cache = fake_cache(&[("http-v2", 10)]);
        let plugin = PipPlugin::with_cache_root(cache.path());

        assert!(plugin
            .scan()
            .expect("scan succeeds")
            .targets
            .iter()
            .all(|t| t.path() != cache.path()));
    }

    #[test]
    fn the_older_http_cache_is_recognised_too() {
        // A machine that upgraded pip can still have the old directory.
        let cache = fake_cache(&[("http", 250)]);
        let plugin = PipPlugin::with_cache_root(cache.path());

        let scan = plugin.scan().expect("scan succeeds");

        assert_eq!(scan.targets.len(), 1);
        assert!(
            scan.total_bytes() >= 250,
            "250 bytes were written, and allocated size rounds up to whole blocks"
        );
    }

    /// The judgement this plugin exists to get right: a downloaded archive is
    /// replaceable by a download, a locally built wheel by a compile.
    #[test]
    fn built_wheels_are_review_and_downloads_are_safe() {
        let cache = fake_cache(&[("http-v2", 10), ("wheels", 10), ("selfcheck", 10)]);
        let plugin = PipPlugin::with_cache_root(cache.path());

        let scan = plugin.scan().expect("scan succeeds");
        let risk_of = |suffix: &str| {
            scan.targets
                .iter()
                .find(|t| t.path().ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix} target"))
                .risk
        };

        assert_eq!(
            risk_of("wheels"),
            TargetRisk::Review,
            "rebuilding a wheel can mean compiling C extensions, not just a download"
        );
        assert_eq!(risk_of("http-v2"), TargetRisk::Safe);
        assert_eq!(risk_of("selfcheck"), TargetRisk::Safe);
        assert!(!risk_of("wheels").may_be_preselected());
    }

    #[test]
    fn an_empty_cache_is_a_successful_scan_with_nothing_in_it() {
        let cache = fake_cache(&[]);
        let plugin = PipPlugin::with_cache_root(cache.path());

        assert!(plugin.scan().expect("not an error").targets.is_empty());
    }

    #[test]
    fn a_missing_cache_directory_reads_as_tool_unavailable() {
        let plugin = PipPlugin::with_cache_root("/nonexistent/sentinel-test/pip-cache");

        let error = plugin.scan().expect_err("a missing cache is not scannable");

        assert!(
            matches!(error, PluginError::ToolUnavailable { .. }),
            "an absent tool must not read as a failure: {error:?}"
        );
    }

    #[test]
    fn recommendations_are_explanations_not_actions() {
        let cache = fake_cache(&[("http-v2", 2048), ("wheels", 4096)]);
        let plugin = PipPlugin::with_cache_root(cache.path());

        let recommendations = plugin.recommend().expect("recommend succeeds");

        assert_eq!(recommendations.len(), 2, "one per risk level present");
        for recommendation in &recommendations {
            assert!(!recommendation.title.is_empty());
            assert!(
                !recommendation
                    .detail
                    .contains(cache.path().to_string_lossy().as_ref()),
                "a recommendation must not carry a path: {recommendation:?}"
            );
        }
    }

    #[test]
    fn estimate_matches_the_scan() {
        let cache = fake_cache(&[("http-v2", 300), ("wheels", 200)]);
        let plugin = PipPlugin::with_cache_root(cache.path());

        let scan = plugin.scan().expect("scan succeeds");
        let estimate = plugin.estimate().expect("estimate succeeds");

        assert_eq!(estimate.reclaimable_bytes, scan.total_bytes());
        assert_eq!(estimate.target_count, scan.targets.len());
    }

    #[test]
    fn scan_then_clean_removes_exactly_what_was_scanned() {
        let cache = fake_cache(&[("http-v2", 100), ("wheels", 50), ("my-notes", 10)]);
        let plugin = PipPlugin::with_cache_root(cache.path());

        let scan = plugin.scan().expect("scan succeeds");
        let outcome = clean(&plugin, &scan.targets).expect("clean succeeds");

        assert_eq!(outcome.removed.len(), 2);
        assert!(
            outcome.reclaimed_bytes >= 150,
            "150 bytes were written, and allocated size rounds up to whole blocks"
        );
        assert!(outcome.failures.is_empty());

        assert!(!cache.path().join("http-v2").exists());
        assert!(!cache.path().join("wheels").exists());
        assert!(
            cache.path().join("my-notes").exists(),
            "an unknown directory must survive a clean"
        );
        assert!(cache.path().exists(), "the cache root itself must survive");
    }

    #[test]
    fn cleaning_a_subset_leaves_the_rest_alone() {
        let cache = fake_cache(&[("http-v2", 100), ("wheels", 50)]);
        let plugin = PipPlugin::with_cache_root(cache.path());

        let scan = plugin.scan().expect("scan succeeds");
        let only_downloads: Vec<CleanTarget> = scan
            .targets
            .iter()
            .filter(|t| t.path().ends_with("http-v2"))
            .cloned()
            .collect();

        clean(&plugin, &only_downloads).expect("clean succeeds");

        assert!(!cache.path().join("http-v2").exists());
        assert!(
            cache.path().join("wheels").exists(),
            "a target the user did not select must not be touched"
        );
    }

    /// The safety contract against the real plugin: a path it never reported
    /// cannot be removed through it.
    #[test]
    fn a_forged_target_is_refused() {
        let cache = fake_cache(&[("http-v2", 10)]);
        let victim = tempfile::tempdir().expect("temp dir");
        fs::write(victim.path().join("precious"), "data").expect("write");

        let plugin = PipPlugin::with_cache_root(cache.path());
        let forged = CleanTarget::new(PLUGIN_ID, victim.path(), "forged", 10, TargetRisk::Safe);

        let outcome = clean(&plugin, &[forged]).expect("refusal is reported, not fatal");

        assert!(outcome.removed.is_empty());
        assert_eq!(outcome.failures.len(), 1);
        assert!(
            victim.path().join("precious").exists(),
            "a path outside the scan must never be removed"
        );
    }

    /// Reads this machine's real pip cache, read-only. Ignored by default; run
    /// with `cargo test -p sentinel-plugin-pip -- --ignored --nocapture` and
    /// cross-check against `du -sb`. It never cleans.
    #[test]
    #[ignore = "reads the developer's real pip cache; run manually"]
    fn scans_the_real_pip_cache() {
        let plugin = PipPlugin::new();

        match plugin.scan() {
            Ok(scan) => {
                println!("pip cache targets ({}):", scan.targets.len());
                for target in &scan.targets {
                    println!(
                        "  {:>10}  {:?}  {}",
                        human_bytes(target.size_bytes),
                        target.risk,
                        target.path().display()
                    );
                    assert_eq!(target.plugin_id(), PLUGIN_ID);
                    assert!(target.path().is_absolute());
                }
                println!("total: {}", human_bytes(scan.total_bytes()));
            }
            Err(PluginError::ToolUnavailable { detail }) => {
                println!("pip not usable here, which is a valid outcome: {detail}");
            }
            Err(other) => panic!("unexpected scan failure: {other}"),
        }
    }

    #[test]
    fn human_bytes_reads_like_du() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1024), "1.0 KiB");
    }
}
