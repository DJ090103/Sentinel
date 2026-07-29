import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/lib/utils";

/**
 * One dashboard metric: a label, a headline value, and optional detail.
 *
 * `value` is a `ReactNode` rather than a string so a card can render "Unknown"
 * in muted type when the OS didn't report a value — the distinction between
 * "no data" and "a real value" should be visible, not flattened into text.
 */
export function StatCard({
  label,
  value,
  detail,
  Icon,
  progressPercent,
}: {
  label: string;
  value: ReactNode;
  detail?: ReactNode;
  Icon: LucideIcon;
  /** When set, draws a 0–100 usage bar under the value. */
  progressPercent?: number;
}) {
  return (
    <Card className="gap-3 py-4">
      <CardHeader className="px-4">
        <CardTitle className="text-muted-foreground flex items-center gap-2 text-xs font-medium">
          <Icon aria-hidden="true" className="size-4" />
          {label}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-2 px-4">
        <p className="truncate text-xl font-semibold tracking-tight">{value}</p>

        {progressPercent !== undefined && (
          <div
            className="bg-muted h-1.5 w-full overflow-hidden rounded-full"
            role="progressbar"
            aria-label={`${label} used`}
            aria-valuenow={Math.round(progressPercent)}
            aria-valuemin={0}
            aria-valuemax={100}
          >
            <div
              className={cn(
                "h-full rounded-full transition-all",
                progressPercent >= 90 ? "bg-destructive" : "bg-primary",
              )}
              style={{ width: `${progressPercent}%` }}
            />
          </div>
        )}

        {detail !== undefined && <p className="text-muted-foreground text-xs">{detail}</p>}
      </CardContent>
    </Card>
  );
}

/** Renders a value that the OS didn't report, so "missing" reads as missing. */
export function UnknownValue() {
  return <span className="text-muted-foreground font-normal">Unknown</span>;
}
