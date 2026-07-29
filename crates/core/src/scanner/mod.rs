//! Runs `scan()` across every registered plugin and merges the results into a
//! single report the UI can render.
//!
//! The important behaviour here is that one plugin cannot spoil the report. A
//! plugin that fails, or that manages a tool this machine doesn't have, is
//! recorded as such and the scan continues — a Docker plugin erroring out
//! should not hide the npm results.

use chrono::{DateTime, Utc};
use sentinel_plugin_api::{PluginError, PluginMetadata, ScanResult};
use serde::Serialize;

use crate::plugins::PluginRegistry;

/// What happened when one plugin was scanned.
///
/// Serialized as a tagged union so the frontend can exhaustively match on
/// `status` instead of inferring state from which optional fields are present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum PluginScanStatus {
    /// The plugin scanned successfully. An empty target list is still a
    /// success — it means there's nothing to clean.
    Completed {
        /// What it found.
        result: ScanResult,
    },
    /// The tool isn't installed, or its cache directory doesn't exist.
    ///
    /// Distinct from `Failed` on purpose: "you don't have Docker" is not an
    /// error the user should be asked to do anything about, and showing it as
    /// one trains people to ignore real failures.
    NotApplicable {
        /// What was looked for and not found.
        detail: String,
    },
    /// The plugin genuinely failed.
    Failed {
        /// Why, in terms the user can act on.
        detail: String,
    },
}

/// One plugin's entry in a scan report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginScanOutcome {
    /// Who was scanned. Carried so the UI can label the row without a second
    /// lookup into the registry.
    pub plugin: PluginMetadata,
    /// How it went.
    pub status: PluginScanStatus,
}

impl PluginScanOutcome {
    /// Bytes this plugin reported as reclaimable, or zero if it didn't complete.
    pub fn reclaimable_bytes(&self) -> u64 {
        match &self.status {
            PluginScanStatus::Completed { result } => result.total_bytes(),
            PluginScanStatus::NotApplicable { .. } | PluginScanStatus::Failed { .. } => 0,
        }
    }
}

/// The merged result of scanning every registered plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    /// When the scan began.
    pub started_at: DateTime<Utc>,
    /// When the last plugin finished.
    pub finished_at: DateTime<Utc>,
    /// One entry per registered plugin, in registration order.
    pub plugins: Vec<PluginScanOutcome>,
}

impl ScanReport {
    /// Total reclaimable space across every plugin that completed.
    pub fn total_reclaimable_bytes(&self) -> u64 {
        self.plugins.iter().fold(0u64, |sum, outcome| {
            sum.saturating_add(outcome.reclaimable_bytes())
        })
    }

    /// Plugins that genuinely failed — excluding ones that merely don't apply
    /// to this machine.
    pub fn failures(&self) -> impl Iterator<Item = &PluginScanOutcome> {
        self.plugins
            .iter()
            .filter(|outcome| matches!(outcome.status, PluginScanStatus::Failed { .. }))
    }
}

/// Scans every registered plugin and merges the results.
///
/// Blocking: plugin scans walk the filesystem (see `sentinel-plugin-api`'s
/// crate docs on why the trait is synchronous). Call this from somewhere that
/// tolerates blocking — the scheduler runs it on a blocking task rather than
/// on an async worker.
pub fn scan_all(registry: &PluginRegistry) -> ScanReport {
    scan_where(registry, |_| true)
}

/// Scans only the plugins a predicate accepts.
///
/// Exists because not every caller wants every plugin. A caller showing one
/// group of tools shouldn't pay for the plugins it isn't going to display —
/// and when a plugin walks the user's own folders rather than a cache
/// directory, "pay for" means real filesystem work, not a rounding error.
///
/// The predicate takes [`PluginMetadata`] rather than a group or category type,
/// so this module needs no opinion about how a caller groups its plugins. That
/// grouping is a presentation decision and lives with the presenter.
///
/// Blocking, for the same reason as [`scan_all`].
pub fn scan_where(
    registry: &PluginRegistry,
    accept: impl Fn(&PluginMetadata) -> bool,
) -> ScanReport {
    let started_at = Utc::now();

    let plugins = registry
        .iter()
        .filter(|plugin| accept(&plugin.metadata()))
        .map(|plugin| {
            let status = match plugin.scan() {
                Ok(result) => PluginScanStatus::Completed { result },
                Err(PluginError::ToolUnavailable { detail }) => {
                    PluginScanStatus::NotApplicable { detail }
                }
                // Rendered via `Display` rather than `Debug` so the UI shows
                // "filesystem operation failed: permission denied" rather than
                // a Rust enum variant.
                Err(other) => PluginScanStatus::Failed {
                    detail: other.to_string(),
                },
            };

            PluginScanOutcome {
                plugin: plugin.metadata(),
                status,
            }
        })
        .collect();

    ScanReport {
        started_at,
        finished_at: Utc::now(),
        plugins,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestPlugin;
    use sentinel_plugin_api::TargetRisk;

    #[test]
    fn empty_registry_produces_an_empty_report() {
        let registry = PluginRegistry::new();

        let report = scan_all(&registry);

        assert!(report.plugins.is_empty());
        assert_eq!(report.total_reclaimable_bytes(), 0);
        assert!(report.finished_at >= report.started_at);
    }

    #[test]
    fn round_trips_a_plugin_through_registry_and_scanner() {
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(TestPlugin::new("alpha").with_target(
                "/tmp/alpha-cache",
                1_024,
                TargetRisk::Safe,
            )))
            .expect("registration succeeds");

        let report = scan_all(&registry);

        assert_eq!(report.plugins.len(), 1);
        let outcome = &report.plugins[0];
        assert_eq!(outcome.plugin.id, "alpha");

        let PluginScanStatus::Completed { result } = &outcome.status else {
            panic!("expected a completed scan, got {:?}", outcome.status);
        };
        assert_eq!(result.targets.len(), 1);
        assert_eq!(result.targets[0].plugin_id(), "alpha");
        assert_eq!(report.total_reclaimable_bytes(), 1_024);
    }

    #[test]
    fn totals_add_up_across_plugins() {
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(TestPlugin::new("alpha").with_target(
                "/tmp/a",
                1_000,
                TargetRisk::Safe,
            )))
            .expect("registration succeeds");
        registry
            .register(Box::new(TestPlugin::new("beta").with_target(
                "/tmp/b",
                2_500,
                TargetRisk::Review,
            )))
            .expect("registration succeeds");

        let report = scan_all(&registry);

        assert_eq!(report.total_reclaimable_bytes(), 3_500);
    }

    /// The behaviour that matters most: a broken plugin must not take the
    /// report down with it.
    #[test]
    fn a_failing_plugin_does_not_hide_the_others() {
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(
                TestPlugin::new("broken").that_fails("disk on fire"),
            ))
            .expect("registration succeeds");
        registry
            .register(Box::new(TestPlugin::new("healthy").with_target(
                "/tmp/h",
                500,
                TargetRisk::Safe,
            )))
            .expect("registration succeeds");

        let report = scan_all(&registry);

        assert_eq!(report.plugins.len(), 2);
        assert_eq!(
            report.total_reclaimable_bytes(),
            500,
            "the healthy plugin's results must still be counted"
        );
        assert_eq!(report.failures().count(), 1);
    }

    #[test]
    fn missing_tool_is_not_reported_as_a_failure() {
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(TestPlugin::new("absent").that_is_unavailable()))
            .expect("registration succeeds");

        let report = scan_all(&registry);

        assert!(
            matches!(
                report.plugins[0].status,
                PluginScanStatus::NotApplicable { .. }
            ),
            "got {:?}",
            report.plugins[0].status
        );
        assert_eq!(
            report.failures().count(),
            0,
            "an uninstalled tool is not a failure"
        );
    }

    #[test]
    fn status_serializes_as_a_tagged_union() {
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(TestPlugin::new("absent").that_is_unavailable()))
            .expect("registration succeeds");

        let json = serde_json::to_string(&scan_all(&registry)).expect("report is plain data");

        assert!(json.contains("\"status\":\"notApplicable\""), "got {json}");
    }
}
