/**
 * TypeScript mirror of the scan and clean types.
 *
 * Rust is the source of truth, as with `system-info.ts` and `events.ts`:
 * `ScanReport` and friends live in `crates/core/src/scanner`, `CleanTarget` and
 * `CleanOutcome` in `crates/plugin-api`, and `ScanSnapshot` /
 * `CleanRequestError` in `apps/desktop/src-tauri/src`. Change those, change
 * these. Rust tests pin the serialized shapes.
 *
 * Note the asymmetry, and that it is deliberate: targets come *out* of Rust and
 * never go back in. `CleanTarget` has no `Deserialize` on the Rust side, so a
 * path cannot enter Sentinel from here at all. Cleaning sends the positions of
 * what the user ticked, which Rust resolves against the scan it still holds.
 */

/**
 * Which page's plugins to scan. Mirrors `ScanSurface`.
 *
 * The split exists because Developer Health scans as soon as it opens — fine
 * for stat-ing known cache directories — while the system plugins walk the
 * user's own folders and must only run when asked.
 */
export type ScanSurface = "developerTools" | "system";

/** How much care a target needs. Mirrors `TargetRisk`. */
export type TargetRisk = "safe" | "review" | "destructive";

/** One removable thing a plugin found. Mirrors `CleanTarget`. */
export interface CleanTarget {
  pluginId: string;
  path: string;
  label: string;
  sizeBytes: number;
  risk: TargetRisk;
}

/** Identity of a plugin. Mirrors `PluginMetadata`. */
export interface PluginMetadata {
  id: string;
  name: string;
  description: string;
}

/**
 * How one plugin's scan went. Mirrors `PluginScanStatus`.
 *
 * `notApplicable` is not a failure — it means the tool isn't installed here.
 * Showing it as an error would train people to ignore the real ones.
 */
export type PluginScanStatus =
  | { status: "completed"; result: { targets: CleanTarget[] } }
  | { status: "notApplicable"; detail: string }
  | { status: "failed"; detail: string };

/** One plugin's entry in a report. Mirrors `PluginScanOutcome`. */
export interface PluginScanOutcome {
  plugin: PluginMetadata;
  status: PluginScanStatus;
}

/** Every plugin's results. Mirrors `ScanReport`. */
export interface ScanReport {
  /** RFC 3339. */
  startedAt: string;
  /** RFC 3339. */
  finishedAt: string;
  plugins: PluginScanOutcome[];
}

/** A scan plus the id a later clean must quote. Mirrors `ScanSnapshot`. */
export interface ScanSnapshot {
  scanId: number;
  report: ScanReport;
}

/**
 * A plain-language explanation. Mirrors `Recommendation`.
 *
 * Note what this does *not* contain: no path, no command, nothing actionable.
 * Recommendations explain; acting means picking targets from a scan.
 */
export interface Recommendation {
  title: string;
  detail: string;
  reclaimableBytes: number;
  risk: TargetRisk;
}

/** One plugin's explanations. Mirrors `PluginRecommendations`. */
export interface PluginRecommendations {
  plugin: PluginMetadata;
  recommendations: Recommendation[];
}

/** One target that couldn't be removed. Mirrors `CleanFailure`. */
export interface CleanFailure {
  label: string;
  reason: string;
}

/** What a clean actually did. Mirrors `CleanOutcome`. */
export interface CleanOutcome {
  removed: string[];
  reclaimedBytes: number;
  failures: CleanFailure[];
}

/** Why a clean request was refused. Mirrors `CleanRequestError`. */
export type CleanRequestError =
  | { kind: "scanExpired" }
  | { kind: "noScan" }
  | { kind: "unknownPlugin"; pluginId: string }
  | { kind: "pluginDidNotComplete"; pluginId: string }
  | { kind: "indexOutOfRange"; index: number; available: number }
  | { kind: "failed"; detail: string };

/** Whether an error from `clean_targets` just means "scan again". */
export function isStaleScan(error: CleanRequestError): boolean {
  return error.kind === "scanExpired" || error.kind === "noScan";
}

/**
 * Turns a rejected clean into something worth showing a user.
 *
 * The structural variants describe a frontend bug rather than anything the user
 * did, so they read as that rather than as gibberish about indices.
 */
export function describeCleanError(error: CleanRequestError): string {
  switch (error.kind) {
    case "scanExpired":
    case "noScan":
      return "These results are out of date. Scan again, then choose what to remove.";
    case "failed":
      return error.detail;
    case "unknownPlugin":
    case "pluginDidNotComplete":
    case "indexOutOfRange":
      return "Sentinel couldn't match your selection to the last scan, so nothing was removed. Scan again.";
  }
}

/** Targets a plugin reported, or an empty list if it didn't complete. */
export function targetsOf(outcome: PluginScanOutcome): CleanTarget[] {
  return outcome.status.status === "completed" ? outcome.status.result.targets : [];
}

/**
 * Whether a target may be ticked for the user in advance.
 *
 * Mirrors `TargetRisk::may_be_preselected`. Only regenerable things are: a
 * pre-ticked box is not a decision, and this is a delete path.
 */
export function mayBePreselected(risk: TargetRisk): boolean {
  return risk === "safe";
}

/**
 * Whether removing this demands a typed confirmation rather than a click.
 *
 * Mirrors `TargetRisk::requires_typed_confirmation`.
 */
export function requiresTypedConfirmation(risk: TargetRisk): boolean {
  return risk === "destructive";
}

/** Short label for a risk level. */
export function riskLabel(risk: TargetRisk): string {
  switch (risk) {
    case "safe":
      return "Regenerable";
    case "review":
      return "Review";
    case "destructive":
      return "Destructive";
  }
}

/**
 * Positions in a plugin's target list that should start ticked.
 *
 * Only regenerable ones. Anything the user would notice losing has to be an
 * opt-in they actually made — a pre-ticked box is not a decision, and both
 * pages that call this delete files.
 */
export function defaultSelection(outcome: PluginScanOutcome): Set<number> {
  const preselected = new Set<number>();
  targetsOf(outcome).forEach((target, index) => {
    if (mayBePreselected(target.risk)) {
      preselected.add(index);
    }
  });
  return preselected;
}
