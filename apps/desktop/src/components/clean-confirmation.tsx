import { AlertTriangle } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import {
  requiresTypedConfirmation,
  riskLabel,
  type CleanTarget,
  type TargetRisk,
} from "@/lib/plugins";
import { formatBytes } from "@/lib/system-info";

/** The word someone must type to confirm a destructive removal. */
const TYPED_CONFIRMATION = "DELETE";

/**
 * The step between choosing what to remove and removing it.
 *
 * Deliberately not a modal: the list of what is about to be deleted stays
 * visible next to the confirmation rather than covering it, and there is no
 * dialog to dismiss with a stray Escape while the mouse is over "Remove".
 *
 * Two levels of friction, matching `TargetRisk`:
 *
 * - Anything regenerable needs one deliberate second click.
 * - Anything `destructive` needs the word typed out, because a click can be
 *   made by muscle memory and typing cannot.
 */
export function CleanConfirmation({
  targets,
  isCleaning,
  onConfirm,
  onCancel,
}: {
  targets: CleanTarget[];
  isCleaning: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const [typed, setTyped] = useState("");

  const totalBytes = targets.reduce((sum, target) => sum + target.sizeBytes, 0);
  const needsTyping = targets.some((target) => requiresTypedConfirmation(target.risk));
  const confirmed = !needsTyping || typed.trim().toUpperCase() === TYPED_CONFIRMATION;

  return (
    <div className="border-destructive/50 bg-destructive/5 space-y-4 rounded-lg border p-4">
      <div className="flex items-start gap-3">
        <AlertTriangle aria-hidden="true" className="text-destructive mt-0.5 size-5 shrink-0" />
        <div className="space-y-1">
          <p className="text-sm font-medium">
            Remove {targets.length} {targets.length === 1 ? "item" : "items"}, freeing{" "}
            {formatBytes(totalBytes)}?
          </p>
          <p className="text-muted-foreground text-sm">
            This deletes the files listed below. It cannot be undone.
          </p>
        </div>
      </div>

      {/* The exact list, again, at the moment of confirming — so what is about
          to happen is on screen next to the button that does it, rather than
          scrolled away above a dialog. */}
      <ul className="space-y-1.5 text-sm">
        {targets.map((target) => (
          <li key={target.path} className="flex flex-wrap items-baseline gap-x-2">
            <RiskBadge risk={target.risk} />
            <span className="font-medium">{target.label.split(" — ")[0]}</span>
            <code className="text-muted-foreground text-xs break-all">{target.path}</code>
            <span className="text-muted-foreground ml-auto tabular-nums">
              {formatBytes(target.sizeBytes)}
            </span>
          </li>
        ))}
      </ul>

      {needsTyping && (
        <div className="space-y-1.5">
          <label htmlFor="typed-confirmation" className="block text-sm font-medium">
            Type <code className="font-semibold">{TYPED_CONFIRMATION}</code> to confirm
          </label>
          <input
            id="typed-confirmation"
            value={typed}
            onChange={(event) => setTyped(event.target.value)}
            autoComplete="off"
            spellCheck={false}
            className="border-input bg-background focus-visible:border-ring focus-visible:ring-ring/50 w-48 rounded-md border px-3 py-1.5 text-sm outline-none focus-visible:ring-[3px]"
          />
        </div>
      )}

      <div className="flex flex-wrap gap-2">
        <Button variant="destructive" disabled={!confirmed || isCleaning} onClick={onConfirm}>
          {isCleaning ? "Removing…" : "Remove permanently"}
        </Button>
        <Button variant="outline" disabled={isCleaning} onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </div>
  );
}

/** A short, colour-coded label for a target's risk. */
export function RiskBadge({ risk }: { risk: TargetRisk }) {
  const tone =
    risk === "safe"
      ? "bg-muted text-muted-foreground"
      : risk === "review"
        ? "bg-amber-500/15 text-amber-700 dark:text-amber-400"
        : "bg-destructive/15 text-destructive";

  return (
    <span className={`rounded px-1.5 py-0.5 text-xs font-medium ${tone}`}>{riskLabel(risk)}</span>
  );
}
