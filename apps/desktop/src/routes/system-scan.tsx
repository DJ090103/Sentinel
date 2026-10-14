import { invoke } from "@tauri-apps/api/core";
import { Info, Search } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { AnalysisReport } from "@/components/analysis-report";
import { CleanConfirmation } from "@/components/clean-confirmation";
import { PageHeader } from "@/components/page-header";
import { CleanResult, PluginSection, ScanSkeleton } from "@/components/plugin-section";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { useHomeAnalysis } from "@/hooks/use-home-analysis";
import { CleanRejected, usePluginScan } from "@/hooks/use-plugin-scan";
import {
  defaultSelection,
  describeCleanError,
  isStaleScan,
  targetsOf,
  type CleanOutcome,
  type CleanRequestError,
  type CleanTarget,
  type PluginRecommendations,
  type PluginScanOutcome,
} from "@/lib/plugins";
import { formatBytes } from "@/lib/system-info";

/**
 * Explanations with no target attached, including the reasoning about `/tmp`.
 *
 * This is the only place a `Recommendation` reaches the screen. Without it the
 * junk plugin's decision not to touch `/tmp` would be an unexplained absence,
 * which a user would reasonably read as a missing feature rather than a choice.
 */
function Explanations({ groups }: { groups: PluginRecommendations[] }) {
  const notes = groups.flatMap((group) => group.recommendations);
  if (notes.length === 0) {
    return null;
  }

  return (
    <Card className="gap-2 py-4">
      <CardContent className="space-y-3 px-4">
        {notes.map((note) => (
          <div key={note.title} className="flex items-start gap-3">
            <Info aria-hidden="true" className="text-muted-foreground mt-0.5 size-4 shrink-0" />
            <div className="min-w-0 space-y-1">
              <p className="text-sm font-medium">{note.title}</p>
              <p className="text-muted-foreground text-sm">{note.detail}</p>
            </div>
          </div>
        ))}
      </CardContent>
    </Card>
  );
}

/**
 * The machine's own disposable data, and the review-then-clean flow for it.
 *
 * # Why this page waits for a button
 *
 * Developer Health scans as soon as it opens, and should — it stats a handful
 * of known cache directories. This page walks the user's own folders: the
 * trash, the screenshots directory. Starting that because someone clicked a
 * sidebar item would be doing work they never asked for, on files that are
 * theirs rather than a tool's. So the page opens empty and explains itself.
 *
 * Everything after the button is the same three deliberate steps as Developer
 * Health — scan, tick, confirm — using the same components, so the delete-path
 * rules can't drift between the two screens.
 */
export function SystemScanPage() {
  const { snapshot, isScanning, error, rescan, clean } = usePluginScan({
    surface: "system",
    scanOnMount: false,
  });

  const home = useHomeAnalysis();
  const [selections, setSelections] = useState<Record<string, Set<number>>>({});
  const [confirming, setConfirming] = useState<string | null>(null);
  const [isCleaning, setIsCleaning] = useState(false);
  const [result, setResult] = useState<CleanOutcome | null>(null);
  const [cleanError, setCleanError] = useState<string | null>(null);
  const [appliedScanId, setAppliedScanId] = useState<number | null>(null);
  const [explanations, setExplanations] = useState<PluginRecommendations[]>([]);

  const hasScanned = snapshot !== null || isScanning;

  // Fetched once a scan has been asked for, not on mount: gathering these runs
  // the plugins' `recommend`, which scans. Loading them eagerly would walk the
  // user's folders behind the button that exists to prevent exactly that.
  useEffect(() => {
    if (!hasScanned) {
      return;
    }

    let cancelled = false;

    void invoke<PluginRecommendations[]>("get_recommendations", { surface: "system" })
      .then((groups) => {
        if (!cancelled) {
          setExplanations(groups);
        }
      })
      // Deliberately silent: an explanation failing to load is not worth an
      // error banner over the scan results, which are the point of the page.
      .catch(() => undefined);

    return () => {
      cancelled = true;
    };
  }, [hasScanned]);

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
      // screen now describes files that are gone.
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
        readOnly
        title="System Scan"
        description="Disposable data that isn't any one tool's — thumbnails, the trash, and screenshots you've forgotten."
      />

      <div className="flex flex-wrap items-center gap-3">
        <Button
          size="sm"
          disabled={isScanning || isCleaning || home.isRunning}
          onClick={() => {
            rescan();
            void home.run();
          }}
        >
          <Search
            aria-hidden="true"
            className={isScanning || home.isRunning ? "animate-pulse" : undefined}
          />
          {isScanning || home.isRunning
            ? "Scanning…"
            : hasScanned
              ? "Scan again"
              : "Scan this machine"}
        </Button>
        {home.isRunning && (
          <Button size="sm" variant="outline" onClick={home.cancel}>
            Cancel
          </Button>
        )}
        {snapshot !== null && !isScanning && (
          <p className="text-muted-foreground text-sm">
            {formatBytes(totals.found)} reclaimable
            {totals.selected > 0 && <> — {formatBytes(totals.selected)} selected</>}
          </p>
        )}
      </div>

      {/* The empty state does real work: it says what will be looked at before
          anything is, which is the whole reason this page waits for a click. */}
      {!hasScanned && (
        <Card className="gap-2 py-6">
          <CardContent className="space-y-3 px-6">
            <h2 className="font-display text-2xl font-extrabold tracking-tight">
              Find out what you can take back
            </h2>
            <p className="text-muted-foreground text-sm">
              Scanning looks in a few places, and reads only — nothing is removed until you tick it
              and confirm:
            </p>
            <ul className="text-muted-foreground list-inside list-disc space-y-1 text-sm">
              <li>Your thumbnail cache, which your file manager rebuilds on its own</li>
              <li>Your trash, which you&rsquo;ve already emptied into once</li>
              <li>Screenshots older than a month, which are yours and are never pre-ticked</li>
              <li>Temporary folders, listing only items untouched for a week</li>
              <li>Your home folder: biggest folders, largest files and duplicates (report only)</li>
            </ul>
            <p className="text-muted-foreground text-sm">
              This is separate from Developer Health because it walks your own folders rather than a
              tool&rsquo;s cache, so it happens when you ask.
            </p>
          </CardContent>
        </Card>
      )}

      {home.isRunning && (
        <p className="text-muted-foreground text-sm" role="status">
          {home.progress?.stage === "duplicates"
            ? `Step 3 of 3 — comparing large files (${home.progress.files.toLocaleString()} checked)`
            : `Step 2 of 3 — reading your home folder (${(home.progress?.files ?? 0).toLocaleString()} files)`}
        </p>
      )}

      {home.error !== null && (
        <div
          role="alert"
          className="border-destructive/50 bg-destructive/10 text-destructive rounded-lg border px-4 py-3 text-sm"
        >
          <p className="font-medium">Couldn&rsquo;t analyze your home folder</p>
          <p className="mt-1 break-words opacity-90">{home.error}</p>
        </div>
      )}

      {error !== null && (
        <div
          role="alert"
          className="border-destructive/50 bg-destructive/10 text-destructive rounded-lg border px-4 py-3 text-sm"
        >
          <p className="font-medium">Couldn&rsquo;t scan this machine</p>
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
              emptyMessage="Nothing disposable found — your thumbnails, trash and screenshots are already tidy."
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

      {hasScanned && <Explanations groups={explanations} />}

      {home.analysis !== null && <AnalysisReport analysis={home.analysis} />}
    </div>
  );
}
