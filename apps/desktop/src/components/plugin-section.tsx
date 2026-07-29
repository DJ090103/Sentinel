import { CheckCircle2 } from "lucide-react";

import { RiskBadge } from "@/components/clean-confirmation";
import { Card, CardContent } from "@/components/ui/card";
import { type CleanOutcome, type PluginScanOutcome } from "@/lib/plugins";
import { formatBytes } from "@/lib/system-info";

/**
 * The pieces both Developer Health and System Scan render.
 *
 * Extracted when the second page arrived rather than copied: these components
 * encode decisions about a delete path — that only regenerable things are
 * pre-ticked, that "not installed" isn't styled as an error, that partial
 * failure is shown rather than hidden. Two copies would be two places for one
 * of those to quietly stop being true.
 */

/** Placeholder shown while a scan is running with nothing to show yet. */
export function ScanSkeleton() {
  return (
    <Card className="gap-3 py-4">
      <CardContent className="space-y-3 px-4">
        <div className="bg-muted h-4 w-32 animate-pulse rounded" />
        <div className="bg-muted h-3 w-64 animate-pulse rounded" />
        <div className="bg-muted h-3 w-48 animate-pulse rounded" />
      </CardContent>
    </Card>
  );
}

/** What happened after a clean, including anything that didn't work. */
export function CleanResult({ outcome }: { outcome: CleanOutcome }) {
  return (
    <div className="space-y-2 rounded-lg border border-emerald-600/40 bg-emerald-500/10 px-4 py-3 text-sm">
      <p className="flex items-center gap-2 font-medium text-emerald-700 dark:text-emerald-400">
        <CheckCircle2 aria-hidden="true" className="size-4" />
        Freed {formatBytes(outcome.reclaimedBytes)}
      </p>
      {outcome.removed.length > 0 && (
        <p className="text-muted-foreground">
          Removed {outcome.removed.length} {outcome.removed.length === 1 ? "item" : "items"}.
        </p>
      )}
      {/* Partial failure is normal, not exceptional — a permission error on one
          directory shouldn't be hidden because the other three worked. */}
      {outcome.failures.length > 0 && (
        <ul className="text-destructive space-y-1">
          {outcome.failures.map((failure) => (
            <li key={failure.label}>
              <span className="font-medium">{failure.label.split(" — ")[0]}</span>: {failure.reason}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** One plugin's results, and the controls for acting on them. */
export function PluginSection({
  outcome,
  selection,
  onToggle,
  disabled,
  emptyMessage = "Nothing to reclaim.",
}: {
  outcome: PluginScanOutcome;
  selection: ReadonlySet<number>;
  onToggle: (index: number) => void;
  disabled: boolean;
  /** Shown when the plugin completed but found nothing. */
  emptyMessage?: string;
}) {
  const { plugin, status } = outcome;

  if (status.status === "notApplicable") {
    return (
      <Card className="gap-2 py-4">
        <CardContent className="space-y-1 px-4">
          <p className="text-sm font-medium">{plugin.name}</p>
          {/* Not styled as an error: not having a tool installed is not a
              problem, and showing it as one teaches people to ignore errors. */}
          <p className="text-muted-foreground text-sm">
            Not in use on this machine — {status.detail}
          </p>
        </CardContent>
      </Card>
    );
  }

  if (status.status === "failed") {
    return (
      <Card className="border-destructive/50 gap-2 py-4">
        <CardContent className="space-y-1 px-4">
          <p className="text-sm font-medium">{plugin.name}</p>
          <p className="text-destructive text-sm">Couldn&rsquo;t scan: {status.detail}</p>
        </CardContent>
      </Card>
    );
  }

  const targets = status.result.targets;
  const total = targets.reduce((sum, target) => sum + target.sizeBytes, 0);

  return (
    <Card className="gap-3 py-4">
      <CardContent className="space-y-3 px-4">
        <div className="flex flex-wrap items-baseline justify-between gap-2">
          <div>
            <p className="text-sm font-medium">{plugin.name}</p>
            <p className="text-muted-foreground text-xs">{plugin.description}</p>
          </div>
          <p className="text-sm font-semibold tabular-nums">{formatBytes(total)}</p>
        </div>

        {targets.length === 0 ? (
          <p className="text-muted-foreground text-sm">{emptyMessage}</p>
        ) : (
          <ul className="space-y-2">
            {targets.map((target, index) => {
              const [name, note] = target.label.split(" — ");
              const inputId = `${plugin.id}-target-${index}`;

              return (
                <li key={target.path}>
                  <div className="flex items-start gap-3">
                    <input
                      id={inputId}
                      type="checkbox"
                      checked={selection.has(index)}
                      disabled={disabled}
                      onChange={() => onToggle(index)}
                      className="accent-primary mt-1 size-4 shrink-0"
                    />
                    <label htmlFor={inputId} className="min-w-0 flex-1 cursor-pointer space-y-0.5">
                      <span className="flex flex-wrap items-center gap-2">
                        <span className="text-sm font-medium">{name}</span>
                        <RiskBadge risk={target.risk} />
                        <span className="text-muted-foreground ml-auto text-sm tabular-nums">
                          {formatBytes(target.sizeBytes)}
                        </span>
                      </span>
                      {note !== undefined && (
                        <span className="text-muted-foreground block text-xs">{note}</span>
                      )}
                    </label>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}
