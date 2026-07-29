//! Sentinel's Bun plugin: reclaims Bun's global install cache.
//!
//! # Two ways this departs from the npm and pip plugins
//!
//! **It runs no subprocess.** `bun pm cache` looks like the right way to ask
//! Bun where its cache is, but it refuses to run outside a directory containing
//! a `package.json` — it is a project command, not a global query. Sentinel has
//! no project to run it in. Bun's layout is well-defined instead, so the path
//! is derived rather than asked for, which removes the only piece of this
//! plugin that would have touched `CLAUDE.md` hard rule 2.
//!
//! **The cache directory is itself the target.** npm and pip have a cache
//! *root* holding several named subdirectories, so those plugins propose the
//! children and never the root. Bun's `install/cache` has no such structure —
//! it is a flat pile of package entries, and the directory is the thing to
//! remove.
//!
//! That is exactly the situation the closed-list rule exists to make safe, so
//! the safety comes from where the path is *derived* rather than from naming
//! children: `<bun home>/install/cache`, where `<bun home>` is `BUN_INSTALL` or
//! `~/.bun`. Both components are constants. A hostile or broken `BUN_INSTALL`
//! of `/` yields `/install/cache`, which does not exist, so the scan finds
//! nothing — the same protection npm and pip get from naming children.
//!
//! `BUN_INSTALL_CACHE_DIR` is deliberately **not** honoured; see
//! [`cache_dir::resolve`].

use std::path::PathBuf;

use sentinel_plugin_api::{
    directory_size, is_empty_directory, CleanFailure, CleanOutcome, CleanTarget, Plugin,
    PluginError, PluginMetadata, Recommendation, ScanResult, TargetRisk, VerifiedTarget,
};

pub mod cache_dir;

/// This plugin's stable id. Stored in cleanup history, so it is permanent.
pub const PLUGIN_ID: &str = "bun";

/// What the user sees for the one target this plugin proposes.
const CACHE_LABEL: &str = "Global install cache";

/// The note shown beside it.
const CACHE_NOTE: &str = "Packages Bun downloaded for `bun install`. Re-downloaded on the next \
                          install that needs them.";

/// The Bun cache plugin.
///
/// Holds no state between calls; the cache location is derived on every scan.
#[derive(Debug, Default)]
pub struct BunPlugin {
    /// Overrides discovery. Exists for tests, which must never touch the
    /// developer's real Bun cache.
    cache_dir_override: Option<PathBuf>,
}

impl BunPlugin {
    /// The plugin as it ships: derives Bun's cache location itself.
    pub fn new() -> Self {
        Self::default()
    }

    /// A plugin pinned to a specific cache directory, for tests.
    pub fn with_cache_dir(path: impl Into<PathBuf>) -> Self {
        Self {
            cache_dir_override: Some(path.into()),
        }
    }

    /// Bun's install cache on this machine.
    fn cache_dir(&self) -> Result<PathBuf, PluginError> {
        match &self.cache_dir_override {
            Some(path) => sentinel_plugin_api::validate_tool_directory(path).map(|()| path.clone()),
            None => cache_dir::resolve(),
        }
    }
}

impl Plugin for BunPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: PLUGIN_ID,
            name: "Bun cache",
            description: "Packages Bun keeps in its global install cache.",
        }
    }

    fn scan(&self) -> Result<ScanResult, PluginError> {
        let cache = self.cache_dir()?;

        if is_empty_directory(&cache) {
            // An empty cache is a successful scan with nothing in it.
            return Ok(ScanResult::default());
        }
        let size = directory_size(&cache)?;

        Ok(ScanResult {
            targets: vec![CleanTarget::new(
                PLUGIN_ID,
                cache,
                format!("{CACHE_LABEL} — {CACHE_NOTE}"),
                size,
                // Purely a download cache: `bun install` refetches whatever it
                // needs. Nothing here is authored, only fetched.
                TargetRisk::Safe,
            )],
        })
    }

    fn recommend(&self) -> Result<Vec<Recommendation>, PluginError> {
        let scan = self.scan()?;
        let bytes = scan.total_bytes();
        if bytes == 0 {
            return Ok(Vec::new());
        }

        Ok(vec![Recommendation {
            title: format!("Bun's install cache is using {}", human_bytes(bytes)),
            detail: "Packages Bun downloaded for previous installs. Clearing them frees the \
                     space now and costs a slower next `bun install`."
                .to_owned(),
            reclaimable_bytes: bytes,
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

    /// A fake Bun cache holding one package entry of the given size.
    fn fake_cache(bytes: usize) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        let entry = dir.path().join("left-pad@1.3.0");
        fs::create_dir_all(&entry).expect("create entry");
        fs::write(entry.join("index.js"), vec![b'x'; bytes]).expect("write");
        dir
    }

    #[test]
    fn metadata_is_stable() {
        assert_eq!(BunPlugin::new().metadata().id, "bun");
        assert_eq!(BunPlugin::new().metadata().id, PLUGIN_ID);
    }

    #[test]
    fn scan_reports_the_cache_as_one_target() {
        let cache = fake_cache(500);
        let plugin = BunPlugin::with_cache_dir(cache.path());

        let scan = plugin.scan().expect("scan succeeds");

        assert_eq!(scan.targets.len(), 1, "Bun's cache is a single directory");
        assert!(
            scan.total_bytes() >= 500,
            "500 bytes were written, and allocated size rounds up to whole blocks"
        );
        assert_eq!(scan.targets[0].plugin_id(), "bun");
        assert_eq!(scan.targets[0].risk, TargetRisk::Safe);
    }

    #[test]
    fn an_empty_cache_offers_nothing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let plugin = BunPlugin::with_cache_dir(dir.path());

        assert!(plugin.scan().expect("not an error").targets.is_empty());
    }

    #[test]
    fn a_missing_cache_reads_as_tool_unavailable() {
        let plugin = BunPlugin::with_cache_dir("/nonexistent/sentinel-test/bun-cache");

        let error = plugin.scan().expect_err("a missing cache is not scannable");

        assert!(
            matches!(error, PluginError::ToolUnavailable { .. }),
            "an absent tool must not read as a failure: {error:?}"
        );
    }

    #[test]
    fn recommendations_are_explanations_not_actions() {
        let cache = fake_cache(4096);
        let plugin = BunPlugin::with_cache_dir(cache.path());

        let recommendations = plugin.recommend().expect("recommend succeeds");

        assert_eq!(recommendations.len(), 1);
        assert!(
            !recommendations[0]
                .detail
                .contains(cache.path().to_string_lossy().as_ref()),
            "a recommendation must not carry a path"
        );
    }

    #[test]
    fn recommend_says_nothing_about_an_empty_cache() {
        let dir = tempfile::tempdir().expect("temp dir");
        let plugin = BunPlugin::with_cache_dir(dir.path());

        assert!(plugin.recommend().expect("recommend succeeds").is_empty());
    }

    #[test]
    fn estimate_matches_the_scan() {
        let cache = fake_cache(750);
        let plugin = BunPlugin::with_cache_dir(cache.path());

        let scan = plugin.scan().expect("scan succeeds");
        let estimate = plugin.estimate().expect("estimate succeeds");

        assert_eq!(estimate.reclaimable_bytes, scan.total_bytes());
        assert_eq!(estimate.target_count, scan.targets.len());
    }

    #[test]
    fn scan_then_clean_removes_the_cache_and_nothing_else() {
        let cache = fake_cache(200);
        let sibling = cache.path().parent().expect("temp parent").join("keep-me");
        fs::create_dir_all(&sibling).expect("create sibling");

        let plugin = BunPlugin::with_cache_dir(cache.path());
        let scan = plugin.scan().expect("scan succeeds");
        let outcome = clean(&plugin, &scan.targets).expect("clean succeeds");

        assert_eq!(outcome.removed.len(), 1);
        assert!(
            outcome.reclaimed_bytes >= 200,
            "200 bytes were written, and allocated size rounds up to whole blocks"
        );
        assert!(!cache.path().exists(), "the cache directory is the target");
        assert!(sibling.exists(), "a sibling directory must be untouched");

        fs::remove_dir_all(&sibling).ok();
    }

    /// The safety contract against the real plugin: a path it never reported
    /// cannot be removed through it.
    #[test]
    fn a_forged_target_is_refused() {
        let cache = fake_cache(10);
        let victim = tempfile::tempdir().expect("temp dir");
        fs::write(victim.path().join("precious"), "data").expect("write");

        let plugin = BunPlugin::with_cache_dir(cache.path());
        let forged = CleanTarget::new(PLUGIN_ID, victim.path(), "forged", 10, TargetRisk::Safe);

        let outcome = clean(&plugin, &[forged]).expect("refusal is reported, not fatal");

        assert!(outcome.removed.is_empty());
        assert_eq!(outcome.failures.len(), 1);
        assert!(
            victim.path().join("precious").exists(),
            "a path outside the scan must never be removed"
        );
    }

    /// Reads this machine's real Bun cache, read-only. Ignored by default.
    #[test]
    #[ignore = "reads the developer's real Bun cache; run manually"]
    fn scans_the_real_bun_cache() {
        match BunPlugin::new().scan() {
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
                println!("Bun not usable here, which is a valid outcome: {detail}");
            }
            Err(other) => panic!("unexpected scan failure: {other}"),
        }
    }

    #[test]
    fn human_bytes_reads_like_du() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1024), "1.0 KiB");
    }
}
