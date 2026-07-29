//! Sentinel's npm plugin: reclaims the caches npm keeps on disk.
//!
//! This is the first real plugin and the worked example every later one copies,
//! so it is deliberately boring and explicit. If something here feels awkward,
//! the trait is the thing to fix — see the Phase 5 guardrail in
//! `product-docs/EXECUTION_PLAN.md` — because a workaround written here gets copied
//! eleven more times.
//!
//! # What it will and won't touch
//!
//! The plugin proposes a **fixed list of named subdirectories** of npm's cache
//! directory, and nothing else. It never proposes the cache directory itself,
//! never walks looking for things that seem cache-like, and never builds a path
//! from anything but a constant joined onto the resolved cache root. Everything
//! it finds is something npm recreates on demand.
//!
//! That list is closed on purpose. An npm release that adds a new cache
//! directory means Sentinel misses some reclaimable space until someone adds it
//! here — which is the right way round. The alternative, deleting whatever
//! turns up inside the cache directory, would mean an npm release that starts
//! storing something durable there turns Sentinel into a data-loss bug.

use std::fs;

use std::path::PathBuf;

use sentinel_plugin_api::{
    directory_size, is_empty_directory, CleanFailure, CleanOutcome, CleanTarget, Plugin,
    PluginError, PluginMetadata, Recommendation, ScanResult, TargetRisk, VerifiedTarget,
};

pub mod cache_dir;

/// This plugin's stable id. Stored in cleanup history, so it is permanent.
pub const PLUGIN_ID: &str = "npm";

/// One directory this plugin knows how to reclaim.
struct KnownCache {
    /// Directory name directly under npm's cache root.
    name: &'static str,
    /// What the user sees.
    label: &'static str,
    /// How much care removing it needs.
    risk: TargetRisk,
    /// Why it is safe, or what is lost — shown alongside the target.
    note: &'static str,
}

/// Every directory this plugin will ever propose.
///
/// `_cacache` is npm's content-addressable store of package tarballs and
/// metadata; `_npx` and `_prebuilds` are download caches for npx packages and
/// native prebuilt binaries. All three are re-fetched on demand.
///
/// `_logs` is the odd one out and is marked [`TargetRisk::Review`] rather than
/// `Safe`: it is not a cache at all but a record of past npm runs, and a record
/// cannot be regenerated. It is small and rarely wanted, but "you can get it
/// back by running the command again" is untrue of it in a way it is true of
/// everything else here.
const KNOWN_CACHES: &[KnownCache] = &[
    KnownCache {
        name: "_cacache",
        label: "Package cache (_cacache)",
        risk: TargetRisk::Safe,
        note: "Downloaded package tarballs and metadata. npm re-fetches what it needs; \
               the next install is slower.",
    },
    KnownCache {
        name: "_npx",
        label: "npx package cache (_npx)",
        risk: TargetRisk::Safe,
        note: "Packages downloaded by `npx`. Re-downloaded the next time you run one.",
    },
    KnownCache {
        name: "_prebuilds",
        label: "Native prebuild cache (_prebuilds)",
        risk: TargetRisk::Safe,
        note: "Prebuilt native binaries. Re-downloaded, or rebuilt from source, on demand.",
    },
    KnownCache {
        name: "_logs",
        label: "Debug logs (_logs)",
        risk: TargetRisk::Review,
        note: "Logs from previous npm runs. Not regenerable — useful only if you are \
               debugging a recent failed install.",
    },
];

/// The npm cache plugin.
///
/// Holds no state between calls. The cache location is resolved on every scan
/// rather than at construction, so moving the cache in `.npmrc` takes effect
/// without restarting Sentinel — and so a plugin constructed before npm is
/// installed starts working once it is.
#[derive(Debug, Default)]
pub struct NpmPlugin {
    /// Overrides discovery. Exists for tests, which must never touch the
    /// developer's real npm cache.
    cache_root_override: Option<PathBuf>,
}

impl NpmPlugin {
    /// The plugin as it ships: discovers npm's cache itself.
    pub fn new() -> Self {
        Self::default()
    }

    /// A plugin pinned to a specific cache directory.
    ///
    /// For tests. Note this does not weaken the safety contract: the path still
    /// goes through the same validation, and the plugin still only proposes
    /// known subdirectories of it that actually exist.
    pub fn with_cache_root(root: impl Into<PathBuf>) -> Self {
        Self {
            cache_root_override: Some(root.into()),
        }
    }

    /// Where npm keeps its cache on this machine.
    fn cache_root(&self) -> Result<PathBuf, PluginError> {
        match &self.cache_root_override {
            Some(root) => cache_dir::validate_cache_dir(root).map(|()| root.clone()),
            None => cache_dir::resolve(),
        }
    }
}

impl Plugin for NpmPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: PLUGIN_ID,
            name: "npm cache",
            description: "Package, npx and prebuild caches npm keeps on disk, plus its debug logs.",
        }
    }

    fn scan(&self) -> Result<ScanResult, PluginError> {
        let root = self.cache_root()?;

        let mut targets = Vec::new();
        for known in KNOWN_CACHES {
            // A constant joined onto a validated root. This is the only place a
            // path is constructed, and there is nothing variable in it.
            let path = root.join(known.name);

            // `symlink_metadata` so a symlink here is seen as a symlink rather
            // than as whatever it points at. Sentinel does not follow links out
            // of the cache directory.
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if !metadata.is_dir() {
                continue;
            }

            if is_empty_directory(&path) {
                // Nothing in it, so nothing to reclaim. Note this is not the same
                // as measuring zero — see `is_empty_directory`.
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

        // One recommendation per risk level rather than per directory: the user
        // is deciding "do I clear regenerable caches?", not making four
        // separate judgements about directories they've never heard of.
        let mut recommendations = Vec::new();

        for risk in [TargetRisk::Safe, TargetRisk::Review] {
            let matching: Vec<&CleanTarget> = scan
                .targets
                .iter()
                .filter(|target| target.risk == risk)
                .collect();

            if matching.is_empty() {
                continue;
            }

            let bytes = matching
                .iter()
                .fold(0u64, |sum, target| sum.saturating_add(target.size_bytes));

            recommendations.push(match risk {
                TargetRisk::Safe => Recommendation {
                    title: format!("npm caches are using {}", human_bytes(bytes)),
                    detail: "These are downloads npm can fetch again. Clearing them frees the \
                             space now and costs a slower next install."
                        .to_owned(),
                    reclaimable_bytes: bytes,
                    risk,
                },
                _ => Recommendation {
                    title: format!("npm debug logs are using {}", human_bytes(bytes)),
                    detail: "Logs from previous npm runs. Safe to remove unless you are \
                             currently debugging a failed install — unlike the caches, these \
                             cannot be regenerated."
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
            // Measured before removal, so `reclaimed_bytes` reports what was
            // actually freed rather than the estimate taken at scan time — the
            // directory may well have grown or shrunk since.
            let freed = directory_size(target.path()).unwrap_or(0);

            match fs::remove_dir_all(target.path()) {
                Ok(()) => {
                    outcome.removed.push(target.label().to_owned());
                    outcome.reclaimed_bytes = outcome.reclaimed_bytes.saturating_add(freed);
                }
                // One failure does not abandon the rest: a permission problem
                // on the log directory should not stop the 900 MiB of package
                // cache next to it from being reclaimed.
                Err(cause) => outcome.failures.push(CleanFailure {
                    label: target.label().to_owned(),
                    reason: cause.to_string(),
                }),
            }
        }

        Ok(outcome)
    }
}

/// Formats bytes for prose in a recommendation.
///
/// Binary units, matching the dashboard — a figure the user can cross-check
/// against `du` is worth more than one that matches a marketing convention.
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

    /// Builds a fake npm cache with the given directories, each holding one
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
        let plugin = NpmPlugin::new();

        assert_eq!(plugin.metadata().id, "npm");
        assert_eq!(plugin.metadata().id, PLUGIN_ID);
    }

    #[test]
    fn scan_reports_known_caches_with_their_sizes() {
        let cache = fake_cache(&[("_cacache", 100), ("_logs", 50)]);
        let plugin = NpmPlugin::with_cache_root(cache.path());

        let scan = plugin.scan().expect("scan succeeds");

        assert_eq!(scan.targets.len(), 2);
        assert!(
            scan.total_bytes() >= 150,
            "150 bytes were written, and allocated size rounds up to whole blocks"
        );
        assert!(scan.targets.iter().all(|t| t.plugin_id() == "npm"));
    }

    /// The closed list is the safety property: anything npm didn't put there
    /// under a name this plugin knows is invisible to it.
    #[test]
    fn scan_ignores_directories_it_does_not_know() {
        let cache = fake_cache(&[("_cacache", 10), ("my-important-work", 999)]);
        let plugin = NpmPlugin::with_cache_root(cache.path());

        let scan = plugin.scan().expect("scan succeeds");

        assert_eq!(scan.targets.len(), 1);
        assert!(
            scan.targets[0].path().ends_with("_cacache"),
            "got {:?}",
            scan.targets[0].path()
        );
    }

    #[test]
    fn scan_never_proposes_the_cache_root_itself() {
        let cache = fake_cache(&[("_cacache", 10)]);
        let plugin = NpmPlugin::with_cache_root(cache.path());

        let scan = plugin.scan().expect("scan succeeds");

        assert!(
            scan.targets.iter().all(|t| t.path() != cache.path()),
            "the cache directory itself must never be a target"
        );
    }

    #[test]
    fn an_empty_cache_is_a_successful_scan_with_nothing_in_it() {
        let cache = fake_cache(&[]);
        let plugin = NpmPlugin::with_cache_root(cache.path());

        let scan = plugin.scan().expect("an empty cache is not an error");

        assert!(scan.targets.is_empty());
        assert_eq!(scan.total_bytes(), 0);
    }

    /// A cache holding no files still holds reclaimable space, and this used to
    /// assert the opposite.
    ///
    /// While `directory_size` measured file lengths, an emptied cache totalled
    /// zero and was skipped, so Sentinel silently declined to offer it. That is
    /// not a hypothetical: this machine's `~/.cache/pip/http-v2` is 2,525 empty
    /// directories holding 9.9 MiB of blocks that a clean would genuinely give
    /// back. Now that sizing counts allocated blocks, the directories are
    /// visible and the target is offered.
    #[cfg(unix)]
    #[test]
    fn a_cache_emptied_of_files_still_has_space_to_reclaim() {
        let dir = tempfile::tempdir().expect("temp dir");
        for index in 0..16 {
            fs::create_dir_all(dir.path().join("_cacache").join(format!("shard-{index}")))
                .expect("create empty dir");
        }
        let plugin = NpmPlugin::with_cache_root(dir.path());

        let scan = plugin.scan().expect("scan succeeds");

        assert_eq!(
            scan.targets.len(),
            1,
            "the emptied cache must still be offered"
        );
        assert!(
            scan.total_bytes() > 0,
            "the directories themselves occupy blocks that removing them frees"
        );
    }

    #[test]
    fn a_missing_cache_directory_reads_as_tool_unavailable() {
        let plugin = NpmPlugin::with_cache_root("/nonexistent/sentinel-test/npm-cache");

        let error = plugin.scan().expect_err("a missing cache is not scannable");

        assert!(
            matches!(error, PluginError::ToolUnavailable { .. }),
            "an absent tool must not read as a failure: {error:?}"
        );
    }

    #[test]
    fn logs_are_review_and_caches_are_safe() {
        let cache = fake_cache(&[("_cacache", 10), ("_logs", 10)]);
        let plugin = NpmPlugin::with_cache_root(cache.path());

        let scan = plugin.scan().expect("scan succeeds");

        let logs = scan
            .targets
            .iter()
            .find(|t| t.path().ends_with("_logs"))
            .expect("logs target");
        let cacache = scan
            .targets
            .iter()
            .find(|t| t.path().ends_with("_cacache"))
            .expect("cacache target");

        assert_eq!(
            logs.risk,
            TargetRisk::Review,
            "logs are a record, not a cache — they can't be regenerated"
        );
        assert_eq!(cacache.risk, TargetRisk::Safe);
        assert!(!logs.risk.may_be_preselected());
    }

    #[test]
    fn recommendations_are_explanations_not_actions() {
        let cache = fake_cache(&[("_cacache", 2048), ("_logs", 10)]);
        let plugin = NpmPlugin::with_cache_root(cache.path());

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
    fn recommend_says_nothing_about_an_empty_cache() {
        let cache = fake_cache(&[]);
        let plugin = NpmPlugin::with_cache_root(cache.path());

        assert!(plugin.recommend().expect("recommend succeeds").is_empty());
    }

    #[test]
    fn estimate_matches_the_scan() {
        let cache = fake_cache(&[("_cacache", 300), ("_npx", 200)]);
        let plugin = NpmPlugin::with_cache_root(cache.path());

        let scan = plugin.scan().expect("scan succeeds");
        let estimate = plugin.estimate().expect("estimate succeeds");

        assert_eq!(estimate.reclaimable_bytes, scan.total_bytes());
        assert_eq!(estimate.target_count, scan.targets.len());
    }

    /// The whole cycle, through the verified entry point rather than by calling
    /// `remove_verified` directly — which is how the app will do it.
    #[test]
    fn scan_then_clean_removes_exactly_what_was_scanned() {
        let cache = fake_cache(&[("_cacache", 100), ("_npx", 50), ("my-work", 10)]);
        let plugin = NpmPlugin::with_cache_root(cache.path());

        let scan = plugin.scan().expect("scan succeeds");
        let outcome = clean(&plugin, &scan.targets).expect("clean succeeds");

        assert_eq!(outcome.removed.len(), 2);
        assert!(
            outcome.reclaimed_bytes >= 150,
            "150 bytes were written, and allocated size rounds up to whole blocks"
        );
        assert!(outcome.failures.is_empty());

        assert!(!cache.path().join("_cacache").exists());
        assert!(!cache.path().join("_npx").exists());
        assert!(
            cache.path().join("my-work").exists(),
            "an unknown directory must survive a clean"
        );
        assert!(cache.path().exists(), "the cache root itself must survive");
    }

    #[test]
    fn cleaning_a_subset_leaves_the_rest_alone() {
        let cache = fake_cache(&[("_cacache", 100), ("_logs", 50)]);
        let plugin = NpmPlugin::with_cache_root(cache.path());

        let scan = plugin.scan().expect("scan succeeds");
        let only_logs: Vec<CleanTarget> = scan
            .targets
            .iter()
            .filter(|t| t.path().ends_with("_logs"))
            .cloned()
            .collect();

        let outcome = clean(&plugin, &only_logs).expect("clean succeeds");

        assert_eq!(outcome.removed.len(), 1);
        assert!(!cache.path().join("_logs").exists());
        assert!(
            cache.path().join("_cacache").exists(),
            "a target the user did not select must not be touched"
        );
    }

    /// The safety contract, exercised against the real plugin rather than a
    /// double: a path this plugin never reported cannot be removed through it.
    #[test]
    fn a_forged_target_is_refused() {
        let cache = fake_cache(&[("_cacache", 10)]);
        let victim = tempfile::tempdir().expect("temp dir");
        fs::write(victim.path().join("precious"), "data").expect("write");

        let plugin = NpmPlugin::with_cache_root(cache.path());
        let forged = CleanTarget::new(PLUGIN_ID, victim.path(), "forged", 10, TargetRisk::Safe);

        let outcome = clean(&plugin, &[forged]).expect("refusal is reported, not fatal");

        assert!(outcome.removed.is_empty());
        assert_eq!(outcome.failures.len(), 1);
        assert!(
            victim.path().join("precious").exists(),
            "a path outside the scan must never be removed"
        );
    }

    /// Scans this machine's real npm cache, read-only.
    ///
    /// Ignored by default: it depends on npm being installed and on whatever
    /// happens to be cached, so it cannot assert exact figures and must not run
    /// in CI. Run it by hand to check discovery and sizing against `du -sh`:
    ///
    /// ```text
    /// cargo test -p sentinel-plugin-npm -- --ignored --nocapture
    /// ```
    ///
    /// It never cleans. Nothing in this file removes anything outside a
    /// `tempfile::TempDir`.
    #[test]
    #[ignore = "reads the developer's real npm cache; run manually"]
    fn scans_the_real_npm_cache() {
        let plugin = NpmPlugin::new();

        match plugin.scan() {
            Ok(scan) => {
                println!("npm cache targets ({}):", scan.targets.len());
                for target in &scan.targets {
                    println!(
                        "  {:>10}  {:?}  {}",
                        human_bytes(target.size_bytes),
                        target.risk,
                        target.path().display()
                    );
                }
                println!("total: {}", human_bytes(scan.total_bytes()));

                for target in &scan.targets {
                    assert_eq!(target.plugin_id(), PLUGIN_ID);
                    assert!(target.path().is_absolute());
                }

                for recommendation in plugin.recommend().expect("recommend succeeds") {
                    println!("- {}", recommendation.title);
                }
            }
            Err(PluginError::ToolUnavailable { detail }) => {
                println!("npm not usable here, which is a valid outcome: {detail}");
            }
            Err(other) => panic!("unexpected scan failure: {other}"),
        }
    }

    #[test]
    fn human_bytes_reads_like_du() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1024), "1.0 KiB");
        assert_eq!(human_bytes(1024 * 1024 * 3 / 2), "1.5 MiB");
    }
}
