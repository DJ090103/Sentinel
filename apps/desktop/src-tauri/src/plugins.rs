//! The set of plugins this build ships with.
//!
//! One place where every plugin crate is named, so "what does Sentinel know
//! how to look at?" has a single answer. Plugins are compiled in — there is no
//! dynamic loading — so this list is the whole story.
//!
//! Nothing here scans or cleans. It builds the registry; `sentinel_core`'s
//! scanner drives it and `sentinel_plugin_api::clean` is the only thing that
//! deletes.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use sentinel_core::plugins::PluginRegistry;
use sentinel_core::scanner::{PluginScanStatus, ScanReport};
use sentinel_plugin_api::CleanTarget;
use sentinel_plugin_browser::BrowserPlugin;
use sentinel_plugin_bun::BunPlugin;
use sentinel_plugin_cargo::CargoPlugin;
use sentinel_plugin_junk::JunkPlugin;
use sentinel_plugin_npm::NpmPlugin;
use sentinel_plugin_pip::PipPlugin;
use sentinel_plugin_vscode::{AntigravityPlugin, CursorPlugin, KiroPlugin, VsCodePlugin};
use serde::{Deserialize, Serialize};

/// Which page a plugin belongs to.
///
/// # Why this exists in `src-tauri` rather than in `plugin-api`
///
/// It is a product decision about where a tool is *shown*, not a property of
/// what a plugin does. `crates/plugin-api` describes the contract for finding
/// and removing files; which screen lists the result is none of its business,
/// and putting it there would mean every plugin author declaring a UI concern.
///
/// # Why the split is needed at all
///
/// Developer Health scans as soon as it opens, which is right for it: stat-ing
/// a handful of known cache directories is fast. The system plugins walk the
/// user's own folders, and that must not happen just because a page was opened.
/// One registry scanned wholesale would make the cheap page pay the expensive
/// page's cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScanSurface {
    /// Caches belonging to installed developer tools. Scanned on page open.
    DeveloperTools,
    /// The machine's own disposable data. Scanned only when asked.
    System,
}

impl ScanSurface {
    /// Whether a plugin id belongs to this surface.
    ///
    /// A match on ids rather than a field on the plugin: adding a plugin to a
    /// page should be a one-line edit next to the registration it belongs
    /// with, not a change to the plugin crate.
    fn owns(self, plugin_id: &str) -> bool {
        let system = matches!(plugin_id, "junk");

        match self {
            Self::System => system,
            Self::DeveloperTools => !system,
        }
    }
}

/// The most recent scan, and the number that identifies it.
struct StoredScan {
    id: u64,
    report: ScanReport,
}

/// The registry, plus the last scan it produced.
///
/// An `Arc` around the registry because scanning walks the filesystem and
/// therefore runs on a blocking thread — the command has to hand an owned
/// handle to that thread, which a borrowed `State` can't provide.
///
/// # Why the scan is kept here at all
///
/// [`CleanTarget`] is deliberately not `Deserialize` (see
/// `sentinel_plugin_api`'s safety contract), so the UI cannot hand targets
/// back. It sends the *positions* of what the user ticked instead, and this is
/// what those positions are resolved against. A path therefore never makes the
/// round trip through the webview, and nothing the frontend sends can name a
/// file that wasn't in a scan Sentinel itself produced.
///
/// # Why there is one stored scan per surface
///
/// The two pages scan independently, and a selection is only meaningful
/// against the exact report it was made from. One shared slot would mean
/// opening Developer Health silently invalidated a System Scan the user was
/// halfway through reviewing. The scan id counter stays *shared*, so ids are
/// unique across both surfaces and a selection can never resolve against the
/// other page's report even by accident.
pub struct PluginState {
    registry: Arc<PluginRegistry>,
    developer_tools_scan: Mutex<Option<StoredScan>>,
    system_scan: Mutex<Option<StoredScan>>,
    next_scan_id: AtomicU64,
}

/// Why a clean request couldn't be turned into a list of targets.
///
/// Serialized as a tagged union so the UI can tell "your list is out of date,
/// scan again" — which is routine and self-correcting — apart from the ones
/// that mean something is actually wrong.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CleanRequestError {
    /// The selection was made against a scan that is no longer the current
    /// one. Refused rather than reinterpreted: positions in an older report
    /// point at different targets in a newer one, so honouring them could
    /// delete something the user never ticked.
    ScanExpired,
    /// No scan has been run yet, so there is nothing to select from.
    NoScan,
    /// The report has no entry for that plugin.
    UnknownPlugin {
        /// The id that was asked for.
        plugin_id: String,
    },
    /// That plugin is in the report but didn't complete, so it has no targets.
    PluginDidNotComplete {
        /// The id that was asked for.
        plugin_id: String,
    },
    /// A position that isn't in that plugin's target list.
    IndexOutOfRange {
        /// The position asked for.
        index: usize,
        /// How many targets that plugin actually reported.
        available: usize,
    },
    /// The removal itself failed outright, as opposed to individual targets
    /// failing — those are reported inside `CleanOutcome::failures`.
    Failed {
        /// What went wrong, in terms the user can act on.
        detail: String,
    },
}

impl fmt::Display for CleanRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScanExpired => write!(
                formatter,
                "these results are out of date — scan again before cleaning"
            ),
            Self::NoScan => write!(formatter, "nothing has been scanned yet"),
            Self::UnknownPlugin { plugin_id } => {
                write!(formatter, "no scan results for plugin `{plugin_id}`")
            }
            Self::PluginDidNotComplete { plugin_id } => {
                write!(formatter, "plugin `{plugin_id}` did not complete its scan")
            }
            Self::IndexOutOfRange { index, available } => write!(
                formatter,
                "selection {index} is outside the {available} results reported"
            ),
            Self::Failed { detail } => write!(formatter, "{detail}"),
        }
    }
}

impl std::error::Error for CleanRequestError {}

impl PluginState {
    /// Builds the registry with every shipped plugin registered.
    ///
    /// A registration failure means two plugins claimed the same id, which with
    /// a fixed in-repo list is a programming error rather than a runtime
    /// condition. It is still reported rather than unwrapped: a released app
    /// should show one fewer tool on the Developer Health page, not fail to
    /// open a window. The registry is always returned, so the commands backed
    /// by it can never hit unmanaged state.
    ///
    /// `screenshot_retention_days` comes from the settings file. It is passed
    /// in rather than read here because this runs before the settings are
    /// loaded unless the caller arranges otherwise — see `lib.rs`, which builds
    /// this inside `setup` for that reason.
    pub fn new(screenshot_retention_days: u32) -> Self {
        let mut registry = PluginRegistry::new();

        if let Err(cause) = registry.register(Box::new(NpmPlugin::new())) {
            eprintln!("Sentinel: could not register the npm plugin: {cause}");
        }
        if let Err(cause) = registry.register(Box::new(PipPlugin::new())) {
            eprintln!("Sentinel: could not register the pip plugin: {cause}");
        }
        if let Err(cause) = registry.register(Box::new(BunPlugin::new())) {
            eprintln!("Sentinel: could not register the bun plugin: {cause}");
        }
        if let Err(cause) = registry.register(Box::new(CargoPlugin::new())) {
            eprintln!("Sentinel: could not register the cargo plugin: {cause}");
        }
        if let Err(cause) = registry.register(Box::new(VsCodePlugin::new())) {
            eprintln!("Sentinel: could not register the VS Code plugin: {cause}");
        }
        if let Err(cause) = registry.register(Box::new(CursorPlugin::new())) {
            eprintln!("Sentinel: could not register the Cursor plugin: {cause}");
        }
        if let Err(cause) = registry.register(Box::new(KiroPlugin::new())) {
            eprintln!("Sentinel: could not register the Kiro plugin: {cause}");
        }
        if let Err(cause) = registry.register(Box::new(AntigravityPlugin::new())) {
            eprintln!("Sentinel: could not register the Antigravity plugin: {cause}");
        }
        if let Err(cause) = registry.register(Box::new(BrowserPlugin::new())) {
            eprintln!("Sentinel: could not register the browser plugin: {cause}");
        }
        // Registered last, and on the System surface rather than Developer
        // Health — see `ScanSurface`.
        if let Err(cause) = registry.register(Box::new(JunkPlugin::new(screenshot_retention_days)))
        {
            eprintln!("Sentinel: could not register the system junk plugin: {cause}");
        }

        Self {
            registry: Arc::new(registry),
            developer_tools_scan: Mutex::new(None),
            system_scan: Mutex::new(None),
            next_scan_id: AtomicU64::new(1),
        }
    }

    /// A second owner of the registry, for work that leaves this thread.
    pub fn shared(&self) -> Arc<PluginRegistry> {
        Arc::clone(&self.registry)
    }

    /// Whether a plugin belongs to the given surface.
    ///
    /// Used by the scan command to filter the registry, so opening one page
    /// never runs the other page's plugins.
    pub fn surface_owns(surface: ScanSurface, plugin_id: &str) -> bool {
        surface.owns(plugin_id)
    }

    /// Stores a scan as the current one for a surface, returning its id.
    ///
    /// Replaces whatever was there for that surface only. Any selection the
    /// user had made against the previous report is invalidated by that, which
    /// is the intent — see [`CleanRequestError::ScanExpired`].
    pub fn record_scan(&self, surface: ScanSurface, report: ScanReport) -> u64 {
        let id = self.next_scan_id.fetch_add(1, Ordering::SeqCst);

        *self.lock_scan(surface) = Some(StoredScan { id, report });

        id
    }

    /// Forgets a surface's current scan.
    ///
    /// Called after a clean: the report describes directories that no longer
    /// exist, and letting a second clean run against it would mean acting on a
    /// list the user reviewed when it described something else.
    pub fn forget_scan(&self, surface: ScanSurface) {
        *self.lock_scan(surface) = None;
    }

    /// Turns positions in a stored scan back into the targets they name.
    ///
    /// Every step is a refusal rather than a best guess. Positions are only
    /// meaningful against the exact report they were chosen from, so a mismatch
    /// anywhere means the request cannot be honoured as intended — and
    /// "cannot be honoured as intended" on a delete path must never resolve to
    /// deleting something else.
    pub fn resolve_targets(
        &self,
        surface: ScanSurface,
        scan_id: u64,
        plugin_id: &str,
        indices: &[usize],
    ) -> Result<Vec<CleanTarget>, CleanRequestError> {
        let stored = self.lock_scan(surface);
        let stored = stored.as_ref().ok_or(CleanRequestError::NoScan)?;

        if stored.id != scan_id {
            return Err(CleanRequestError::ScanExpired);
        }

        let outcome = stored
            .report
            .plugins
            .iter()
            .find(|entry| entry.plugin.id == plugin_id)
            .ok_or_else(|| CleanRequestError::UnknownPlugin {
                plugin_id: plugin_id.to_owned(),
            })?;

        let PluginScanStatus::Completed { result } = &outcome.status else {
            return Err(CleanRequestError::PluginDidNotComplete {
                plugin_id: plugin_id.to_owned(),
            });
        };

        indices
            .iter()
            .map(|index| {
                result
                    .targets
                    .get(*index)
                    .cloned()
                    .ok_or(CleanRequestError::IndexOutOfRange {
                        index: *index,
                        available: result.targets.len(),
                    })
            })
            .collect()
    }

    /// A surface's stored scan, recovering from a poisoned lock.
    ///
    /// A panic while this was held would have left a `ScanReport` behind, which
    /// is inert data — nothing a panic can make inconsistent. Refusing every
    /// later scan because of one earlier panic would be the worse outcome.
    fn lock_scan(&self, surface: ScanSurface) -> std::sync::MutexGuard<'_, Option<StoredScan>> {
        match surface {
            ScanSurface::DeveloperTools => &self.developer_tools_scan,
            ScanSurface::System => &self.system_scan,
        }
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Default for PluginState {
    fn default() -> Self {
        Self::new(sentinel_core::settings::DEFAULT_SCREENSHOT_RETENTION_DAYS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sentinel_core::scanner;
    use sentinel_plugin_api::{Plugin, PluginError, PluginMetadata, Recommendation, ScanResult};

    /// The surface these tests record scans against. Which one is arbitrary —
    /// the behaviour under test is the same for both — except in
    /// `a_selection_is_refused_by_the_other_surface`, which needs both.
    const SURFACE: ScanSurface = ScanSurface::DeveloperTools;

    #[test]
    fn every_shipped_plugin_registers() {
        let state = PluginState::default();

        // If this count and the registration list above drift apart, a plugin
        // was added to one and not the other — which shows up as a tool
        // silently missing from the UI rather than as a build failure.
        assert_eq!(state.registry.len(), 10, "registered: {:?}", state.registry);
        for id in [
            "npm",
            "pip",
            "bun",
            "cargo",
            "vscode",
            "cursor",
            "kiro",
            "antigravity",
            "browser",
            "junk",
        ] {
            assert!(
                state.registry.get(id).is_some(),
                "`{id}` must be reachable by the id its cleanup history is keyed on"
            );
        }
    }

    /// A plugin reporting a fixed set of targets, so resolution can be tested
    /// without touching a filesystem.
    struct FakePlugin;

    impl Plugin for FakePlugin {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata {
                id: "fake",
                name: "Fake",
                description: "Test double.",
            }
        }

        fn scan(&self) -> Result<ScanResult, PluginError> {
            Ok(ScanResult {
                targets: vec![
                    CleanTarget::new(
                        "fake",
                        "/tmp/one",
                        "one",
                        1,
                        sentinel_plugin_api::TargetRisk::Safe,
                    ),
                    CleanTarget::new(
                        "fake",
                        "/tmp/two",
                        "two",
                        2,
                        sentinel_plugin_api::TargetRisk::Safe,
                    ),
                ],
            })
        }

        fn recommend(&self) -> Result<Vec<Recommendation>, PluginError> {
            Ok(Vec::new())
        }

        fn remove_verified(
            &self,
            _verified: &[sentinel_plugin_api::VerifiedTarget<'_>],
        ) -> Result<sentinel_plugin_api::CleanOutcome, PluginError> {
            Ok(sentinel_plugin_api::CleanOutcome::default())
        }
    }

    fn state_with_a_recorded_scan() -> (PluginState, u64) {
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(FakePlugin))
            .expect("registration succeeds");

        let state = PluginState {
            registry: Arc::new(registry),
            developer_tools_scan: Mutex::new(None),
            system_scan: Mutex::new(None),
            next_scan_id: AtomicU64::new(1),
        };

        let report = scanner::scan_all(&state.registry);
        let id = state.record_scan(SURFACE, report);

        (state, id)
    }

    #[test]
    fn positions_resolve_to_the_targets_they_name() {
        let (state, scan_id) = state_with_a_recorded_scan();

        let targets = state
            .resolve_targets(SURFACE, scan_id, "fake", &[1])
            .expect("a valid selection resolves");

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].label, "two");
    }

    /// The guard that matters: positions chosen against one report point at
    /// different things in the next, so an older id must never be honoured.
    #[test]
    fn a_selection_from_a_superseded_scan_is_refused() {
        let (state, first_scan) = state_with_a_recorded_scan();
        let second_report = scanner::scan_all(&state.registry);
        state.record_scan(SURFACE, second_report);

        let error = state
            .resolve_targets(SURFACE, first_scan, "fake", &[0])
            .expect_err("a stale scan id must be refused");

        assert_eq!(error, CleanRequestError::ScanExpired);
    }

    /// The two pages hold their scans separately, so a selection made on one
    /// must not resolve against the other's report. Ids come from a shared
    /// counter precisely so this can never collide by accident.
    #[test]
    fn a_selection_is_refused_by_the_other_surface() {
        let (state, developer_scan) = state_with_a_recorded_scan();

        let error = state
            .resolve_targets(ScanSurface::System, developer_scan, "fake", &[0])
            .expect_err("a Developer Health selection must not resolve on System Scan");

        assert_eq!(
            error,
            CleanRequestError::NoScan,
            "the system surface has no scan of its own"
        );

        // And with a scan on *both* surfaces, the ids still don't cross.
        let system_report = scanner::scan_all(&state.registry);
        let system_scan = state.record_scan(ScanSurface::System, system_report);
        assert_ne!(
            developer_scan, system_scan,
            "ids are drawn from one counter"
        );

        assert_eq!(
            state.resolve_targets(ScanSurface::System, developer_scan, "fake", &[0]),
            Err(CleanRequestError::ScanExpired)
        );
    }

    /// Which plugins each page runs. The system plugins walk the user's own
    /// folders, so one landing on Developer Health would mean opening that page
    /// silently started that work.
    #[test]
    fn each_surface_owns_the_right_plugins() {
        for tool in ["npm", "pip", "bun", "cargo", "vscode", "cursor", "browser"] {
            assert!(PluginState::surface_owns(ScanSurface::DeveloperTools, tool));
            assert!(!PluginState::surface_owns(ScanSurface::System, tool));
        }

        assert!(PluginState::surface_owns(ScanSurface::System, "junk"));
        assert!(
            !PluginState::surface_owns(ScanSurface::DeveloperTools, "junk"),
            "Developer Health scans on open; it must not walk the user's folders"
        );
    }

    #[test]
    fn a_selection_after_a_clean_is_refused() {
        let (state, scan_id) = state_with_a_recorded_scan();
        state.forget_scan(SURFACE);

        assert_eq!(
            state.resolve_targets(SURFACE, scan_id, "fake", &[0]),
            Err(CleanRequestError::NoScan)
        );
    }

    #[test]
    fn a_position_past_the_end_is_refused() {
        let (state, scan_id) = state_with_a_recorded_scan();

        assert_eq!(
            state.resolve_targets(SURFACE, scan_id, "fake", &[9]),
            Err(CleanRequestError::IndexOutOfRange {
                index: 9,
                available: 2
            })
        );
    }

    #[test]
    fn an_unknown_plugin_is_refused() {
        let (state, scan_id) = state_with_a_recorded_scan();

        assert_eq!(
            state.resolve_targets(SURFACE, scan_id, "not-a-plugin", &[0]),
            Err(CleanRequestError::UnknownPlugin {
                plugin_id: "not-a-plugin".to_owned()
            })
        );
    }

    #[test]
    fn one_bad_position_refuses_the_whole_selection() {
        let (state, scan_id) = state_with_a_recorded_scan();

        assert!(
            state
                .resolve_targets(SURFACE, scan_id, "fake", &[0, 9])
                .is_err(),
            "a partially resolvable selection must not be partially honoured"
        );
    }

    #[test]
    fn the_npm_plugin_describes_itself_for_the_ui() {
        let state = PluginState::default();
        let metadata = state.registry.metadata();

        let npm = metadata
            .iter()
            .find(|entry| entry.id == "npm")
            .expect("npm is registered");

        assert!(!npm.name.is_empty());
        assert!(
            !npm.description.is_empty(),
            "the Developer Health page lists this before any scan runs"
        );
    }
}
