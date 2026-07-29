//! Scanning developer tools, and cleaning what the user picked from a scan.

use sentinel_core::scanner::{self, ScanReport};
use sentinel_plugin_api::{CleanOutcome, PluginMetadata, Recommendation};
use serde::Serialize;
use tauri::State;

use crate::plugins::{CleanRequestError, PluginState, ScanSurface};

/// A scan, plus the number that identifies it.
///
/// The id exists so a later clean can prove it is acting on the results the
/// user actually reviewed. Positions in an older report point at different
/// targets in a newer one, so without it a rescan between review and confirm
/// could silently redirect a deletion.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSnapshot {
    /// Echo this back when cleaning.
    pub scan_id: u64,
    /// What every registered plugin found.
    pub report: ScanReport,
}

/// Scans every registered plugin and returns one merged report.
///
/// # Why this is `async` when the work is blocking
///
/// Tauri dispatches commands onto its async runtime, so a synchronous body
/// would occupy a runtime worker for the whole scan — and a scan walks every
/// file in every cache directory it knows about, which is hundreds of
/// thousands of files and takes a noticeable fraction of a second. Handing that
/// to `spawn_blocking` keeps it off the async workers, which is where the rest
/// of the app's IPC is being served.
///
/// # Errors
///
/// Only if the blocking task itself fails to run or panics. A *plugin* failing
/// is not an error here: `scan_all` records it against that plugin and carries
/// on, so one broken tool can't hide the others. The report distinguishes "this
/// tool isn't installed" from "this tool failed", and the UI should too.
/// Scans one surface's plugins.
///
/// `surface` decides which: Developer Health asks for `developerTools` and gets
/// the tool caches, System Scan asks for `system` and gets the machine's own
/// disposable data. Opening one page therefore never runs the other's plugins —
/// which matters because the system plugins walk the user's own folders and
/// must not do so merely because a page was opened.
#[tauri::command]
pub async fn scan_plugins(
    state: State<'_, PluginState>,
    surface: ScanSurface,
) -> Result<ScanSnapshot, String> {
    let registry = state.shared();

    let report = tauri::async_runtime::spawn_blocking(move || {
        scanner::scan_where(&registry, |plugin| {
            PluginState::surface_owns(surface, plugin.id)
        })
    })
    .await
    // Rendered rather than propagated: the frontend can't do anything with
    // a `JoinError`, and this only happens if the scan panicked, which is a
    // bug worth surfacing as text.
    .map_err(|cause| format!("the scan did not complete: {cause}"))?;

    let scan_id = state.record_scan(surface, report.clone());

    Ok(ScanSnapshot { scan_id, report })
}

/// One plugin's recommendations, with the plugin that made them.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRecommendations {
    /// Who is explaining. Carried so the UI can attribute the text.
    pub plugin: PluginMetadata,
    /// What it has to say. Explanations only — a `Recommendation` carries no
    /// path and nothing executable, by design.
    pub recommendations: Vec<Recommendation>,
}

/// Plain-language explanations from a surface's plugins.
///
/// Separate from `scan_plugins` because it answers a different question. A scan
/// says *what could be removed*; this says *what is going on, including about
/// things Sentinel will not offer to remove at all.* The junk plugin's
/// explanation of why it leaves `/tmp` alone has no target attached to it and
/// would therefore be invisible in a scan report — an absence the user would
/// reasonably read as a missing feature rather than a decision.
///
/// A plugin that fails is skipped rather than failing the call: missing an
/// explanation is a much smaller problem than showing none of them.
#[tauri::command]
pub async fn get_recommendations(
    state: State<'_, PluginState>,
    surface: ScanSurface,
) -> Result<Vec<PluginRecommendations>, String> {
    let registry = state.shared();

    // Blocking for the same reason `scan_plugins` is: the default `recommend`
    // implementation runs a scan, so this walks the filesystem too.
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .iter()
            .filter(|plugin| PluginState::surface_owns(surface, plugin.metadata().id))
            .filter_map(|plugin| {
                let recommendations = plugin.recommend().ok()?;
                if recommendations.is_empty() {
                    return None;
                }

                Some(PluginRecommendations {
                    plugin: plugin.metadata(),
                    recommendations,
                })
            })
            .collect()
    })
    .await
    .map_err(|cause| format!("the explanations could not be gathered: {cause}"))
}

/// Removes the targets the user selected from a scan.
///
/// Takes *positions* in a stored scan rather than the targets themselves.
/// `CleanTarget` is deliberately not `Deserialize` (see `sentinel_plugin_api`'s
/// safety contract), so a path cannot enter Sentinel from the webview at all —
/// the frontend can only point at something Sentinel already found.
///
/// Everything that actually deletes is behind `sentinel_plugin_api::clean`,
/// which re-verifies each target against a fresh scan before touching it. So
/// there are two independent checks between a click and a deletion: this one
/// proves the selection refers to results the user reviewed, and that one
/// proves those results are still true.
///
/// The stored scan is discarded afterwards, successfully or not. It now
/// describes directories that may no longer exist, and a second clean against
/// it would be acting on a list that was reviewed when it meant something else.
#[tauri::command]
pub async fn clean_targets(
    state: State<'_, PluginState>,
    surface: ScanSurface,
    scan_id: u64,
    plugin_id: String,
    target_indices: Vec<usize>,
) -> Result<CleanOutcome, CleanRequestError> {
    let targets = state.resolve_targets(surface, scan_id, &plugin_id, &target_indices)?;
    let registry = state.shared();

    // Dropped before any deletion, not after: if this process dies mid-clean,
    // the next launch should start from a fresh scan rather than from a list
    // describing directories that are now half-removed.
    state.forget_scan(surface);

    let plugin_id_for_task = plugin_id.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let plugin = registry
            .get(&plugin_id_for_task)
            .ok_or(CleanRequestError::UnknownPlugin {
                plugin_id: plugin_id_for_task.clone(),
            })?;

        sentinel_plugin_api::clean(plugin, &targets).map_err(|cause| CleanRequestError::Failed {
            detail: cause.to_string(),
        })
    })
    .await
    .map_err(|cause| CleanRequestError::Failed {
        detail: format!("the clean did not complete: {cause}"),
    })??;

    Ok(outcome)
}
