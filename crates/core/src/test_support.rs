//! Test doubles shared across this crate's unit tests.
//!
//! Compiled only under `cfg(test)` and never shipped. Phase 3 of
//! `product-docs/EXECUTION_PLAN.md` deliberately proves the trait, registry, and
//! scanner against a throwaway implementation rather than against a real
//! plugin — committing to the trait's shape before anything depends on it is
//! the whole point of doing this phase separately.

use std::path::PathBuf;

use sentinel_plugin_api::{
    clean, CleanOutcome, CleanTarget, Plugin, PluginError, PluginMetadata, Recommendation,
    ScanResult, TargetRisk, VerifiedTarget,
};

/// How the double should behave when scanned.
enum Behaviour {
    /// Return the configured targets.
    Succeed,
    /// Report the managed tool as absent.
    Unavailable,
    /// Fail with the given message.
    Fail(String),
}

/// A configurable `Plugin` implementation for exercising the orchestration.
pub struct TestPlugin {
    id: &'static str,
    behaviour: Behaviour,
    targets: Vec<CleanTarget>,
}

impl TestPlugin {
    /// A plugin with the given id that scans successfully and finds nothing.
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            behaviour: Behaviour::Succeed,
            targets: Vec::new(),
        }
    }

    /// Adds a target this plugin will report.
    pub fn with_target(
        mut self,
        path: impl Into<PathBuf>,
        size_bytes: u64,
        risk: TargetRisk,
    ) -> Self {
        let path = path.into();
        let label = format!("{} target at {}", self.id, path.display());
        self.targets
            .push(CleanTarget::new(self.id, path, label, size_bytes, risk));
        self
    }

    /// Makes `scan` fail.
    pub fn that_fails(mut self, detail: &str) -> Self {
        self.behaviour = Behaviour::Fail(detail.to_owned());
        self
    }

    /// Makes `scan` report the managed tool as not installed.
    pub fn that_is_unavailable(mut self) -> Self {
        self.behaviour = Behaviour::Unavailable;
        self
    }
}

impl Plugin for TestPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: self.id,
            name: "Test plugin",
            description: "A test double. Never registered outside tests.",
        }
    }

    fn scan(&self) -> Result<ScanResult, PluginError> {
        match &self.behaviour {
            Behaviour::Succeed => Ok(ScanResult {
                targets: self.targets.clone(),
            }),
            Behaviour::Unavailable => Err(PluginError::ToolUnavailable {
                detail: format!("{} is not installed", self.id),
            }),
            Behaviour::Fail(detail) => Err(PluginError::Other {
                detail: detail.clone(),
            }),
        }
    }

    fn recommend(&self) -> Result<Vec<Recommendation>, PluginError> {
        let scan = self.scan()?;
        if scan.targets.is_empty() {
            return Ok(Vec::new());
        }

        Ok(vec![Recommendation {
            title: format!("{} has reclaimable data", self.id),
            detail: "Test recommendation. Explanation only, never an action.".to_owned(),
            reclaimable_bytes: scan.total_bytes(),
            risk: TargetRisk::Safe,
        }])
    }

    /// Pretends to delete, and reports it.
    ///
    /// Note what isn't here: no id check and no re-scan. The free function
    /// `clean` did both before this was called, and a `VerifiedTarget` can't be built any
    /// other way — which is exactly the property the trait change was for. This
    /// double is now the same shape a real plugin will be.
    fn remove_verified(
        &self,
        verified: &[VerifiedTarget<'_>],
    ) -> Result<CleanOutcome, PluginError> {
        let mut outcome = CleanOutcome::default();

        for target in verified {
            outcome.removed.push(target.label().to_owned());
            outcome.reclaimed_bytes = outcome.reclaimed_bytes.saturating_add(target.size_bytes());
        }

        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_refuses_targets_belonging_to_another_plugin() {
        let plugin = TestPlugin::new("alpha").with_target("/tmp/a", 10, TargetRisk::Safe);
        let foreign = CleanTarget::new("beta", "/tmp/b", "beta's target", 10, TargetRisk::Safe);

        let error =
            clean(&plugin, &[foreign]).expect_err("a target from another plugin must be refused");

        assert!(
            matches!(error, PluginError::RejectedTarget { .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn clean_refuses_a_target_absent_from_a_fresh_scan() {
        let plugin = TestPlugin::new("alpha").with_target("/tmp/real", 10, TargetRisk::Safe);
        // Same plugin id, but a path this plugin never reported — the shape a
        // stale or tampered-with review list would take.
        let stale = CleanTarget::new("alpha", "/tmp/gone", "stale target", 10, TargetRisk::Safe);

        let outcome = clean(&plugin, &[stale]).expect("mismatch is reported, not fatal");

        assert!(outcome.removed.is_empty(), "nothing should be removed");
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(outcome.reclaimed_bytes, 0);
    }

    #[test]
    fn clean_accepts_its_own_current_targets() {
        let plugin = TestPlugin::new("alpha").with_target("/tmp/real", 64, TargetRisk::Safe);
        let scanned = plugin.scan().expect("scan succeeds");

        let outcome = clean(&plugin, &scanned.targets).expect("own targets are accepted");

        assert_eq!(outcome.removed.len(), 1);
        assert_eq!(outcome.reclaimed_bytes, 64);
        assert!(outcome.failures.is_empty());
    }

    #[test]
    fn estimate_defaults_to_totalling_a_scan() {
        let plugin = TestPlugin::new("alpha")
            .with_target("/tmp/a", 100, TargetRisk::Safe)
            .with_target("/tmp/b", 200, TargetRisk::Review);

        let estimate = plugin.estimate().expect("estimate succeeds");

        assert_eq!(estimate.reclaimable_bytes, 300);
        assert_eq!(estimate.target_count, 2);
    }

    #[test]
    fn recommend_says_nothing_when_there_is_nothing_to_say() {
        let plugin = TestPlugin::new("alpha");

        assert!(plugin.recommend().expect("recommend succeeds").is_empty());
    }
}
