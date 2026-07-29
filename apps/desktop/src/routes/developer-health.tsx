import { RefreshCw } from "lucide-react";
import { useMemo, useState } from "react";

import { CleanConfirmation } from "@/components/clean-confirmation";
import { PageHeader } from "@/components/page-header";
import { CleanResult, PluginSection, ScanSkeleton } from "@/components/plugin-section";
import { Button } from "@/components/ui/button";
import { CleanRejected, usePluginScan } from "@/hooks/use-plugin-scan";
import {
  defaultSelection,
  describeCleanError,
  isStaleScan,
  targetsOf,
  type CleanOutcome,
  type CleanRequestError,
  type CleanTarget,
  type PluginScanOutcome,
} from "@/lib/plugins";
import { formatBytes } from "@/lib/system-info";

/**
 * Per-tool view: what each developer tool is costing, and the review-then-clean
 * flow for reclaiming it.
 *
 * The flow is deliberately three steps — scan, tick, confirm — and never fewer.
 * Nothing is removed by a single click, and nothing the user would notice
 * losing is ticked for them.
 */
export function DeveloperHealthPage() {
  // Scans on open, unlike System Scan: this only stats known cache
  // directories, and the page's whole purpose is showing what they cost.
  const { snapshot, isScanning, error, rescan, clean } = usePluginScan({
    surface: "developerTools",
  });

  // Keyed by plugin id. Rebuilt whenever a new scan arrives, because positions
  // in the old report mean something different in the new one.
  const [selections, setSelections] = useState<Record<string, Set<number>>>({});
  const [confirming, setConfirming] = useState<string | null>(null);
  const [isCleaning, setIsCleaning] = useState(false);
  const [result, setResult] = useState<CleanOutcome | null>(null);
  const [cleanError, setCleanError] = useState<string | null>(null);
  const [appliedScanId, setAppliedScanId] = useState<number | null>(null);

  // Reset selections when a scan lands. Derived during render rather than in an
  // effect, so there is never a frame where the previous scan's ticks sit
  // against the new scan's rows.
  if (snapshot !== null && snapshot.scanId !== appliedScanId) {
    setAppliedScanId(snapshot.scanId);
    setSelections(
      Object.fromEntries(
        snapshot.report.plugins.map((entry) => [entry.plugin.id, defaultSelection(entry)]),
      ),
    );
    setConfirming(null);
    setCleanError(null);
  }

  const totals = useMemo(() => {
    if (snapshot === null) {
      return { found: 0, selected: 0 };
    }

    let found = 0;
    let selected = 0;

    for (const outcome of snapshot.report.plugins) {
      const picked = selections[outcome.plugin.id] ?? new Set<number>();
      targetsOf(outcome).forEach((target, index) => {
        found += target.sizeBytes;
        if (picked.has(index)) {
          selected += target.sizeBytes;
        }
      });
    }

    return { found, selected };
  }, [snapshot, selections]);

  const toggle = (pluginId: string, index: number) => {
    setSelections((current) => {
      const next = new Set(current[pluginId] ?? []);
      if (!next.delete(index)) {
        next.add(index);
      }
      return { ...current, [pluginId]: next };
    });
  };

  const selectedTargets = (outcome: PluginScanOutcome): CleanTarget[] => {
    const picked = selections[outcome.plugin.id] ?? new Set<number>();
    return targetsOf(outcome).filter((_, index) => picked.has(index));
  };

  const runClean = async (pluginId: string) => {
    const indices = [...(selections[pluginId] ?? [])].sort((a, b) => a - b);

    setIsCleaning(true);
    setCleanError(null);

    try {
      setResult(await clean(pluginId, indices));
      setConfirming(null);
      // The stored scan is spent — the backend discarded it, and the report on
      // screen now describes directories that are gone.
      rescan();
    } catch (cause) {
      const rejection: CleanRequestError =
        cause instanceof CleanRejected ? cause.reason : { kind: "failed", detail: String(cause) };

      setCleanError(describeCleanError(rejection));
      setConfirming(null);

      if (isStaleScan(rejection)) {
        rescan();
      }
    } finally {
      setIsCleaning(false);
    }
  };

  return (
    <div className="space-y-6">
      <PageHeader
        title="Developer Health"
        description="What your toolchain is costing in disk space and cache."
      />

      <div className="flex flex-wrap items-center gap-3">
        <Button variant="outline" size="sm" disabled={isScanning || isCleaning} onClick={rescan}>
          <RefreshCw aria-hidden="true" className={isScanning ? "animate-spin" : undefined} />
          {isScanning ? "Scanning…" : "Scan again"}
        </Button>
        {snapshot !== null && !isScanning && (
          <p className="text-muted-foreground text-sm">
            {formatBytes(totals.found)} reclaimable
            {totals.selected > 0 && <> — {formatBytes(totals.selected)} selected</>}
          </p>
        )}
      </div>

      {error !== null && (
        <div
          role="alert"
          className="border-destructive/50 bg-destructive/10 text-destructive rounded-lg border px-4 py-3 text-sm"
        >
          <p className="font-medium">Couldn&rsquo;t scan your tools</p>
          <p className="mt-1 break-words opacity-90">{error}</p>
        </div>
      )}

      {cleanError !== null && (
        <div
          role="alert"
          className="border-destructive/50 bg-destructive/10 text-destructive rounded-lg border px-4 py-3 text-sm"
        >
          <p className="font-medium">Nothing was removed</p>
          <p className="mt-1 break-words opacity-90">{cleanError}</p>
        </div>
      )}

      {result !== null && <CleanResult outcome={result} />}

      {snapshot === null && isScanning && <ScanSkeleton />}

      {snapshot?.report.plugins.map((outcome) => {
        const pluginId = outcome.plugin.id;
        const picked = selections[pluginId] ?? new Set<number>();
        const chosen = selectedTargets(outcome);

        return (
          <div key={pluginId} className="space-y-3">
            <PluginSection
              outcome={outcome}
              selection={picked}
              onToggle={(index) => toggle(pluginId, index)}
              disabled={isCleaning || confirming === pluginId}
            />

            {confirming === pluginId ? (
              <CleanConfirmation
                targets={chosen}
                isCleaning={isCleaning}
                onConfirm={() => void runClean(pluginId)}
                onCancel={() => setConfirming(null)}
              />
            ) : (
              chosen.length > 0 && (
                <Button
                  variant="destructive"
                  size="sm"
                  disabled={isScanning || isCleaning}
                  onClick={() => {
                    setResult(null);
                    setConfirming(pluginId);
                  }}
                >
                  Review {chosen.length} {chosen.length === 1 ? "item" : "items"} to remove
                </Button>
              )
            )}
          </div>
        );
      })}
    </div>
  );
}
