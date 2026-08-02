import { PageHeader } from "@/components/page-header";

/**
 * Trends over time, and a log of what Sentinel has cleaned.
 *
 * A stub until a reviewed persistence design records snapshots; there is
 * nothing honest to chart while Sentinel stores no time series.
 */
export function InsightsPage() {
  return (
    <div className="space-y-6">
      <PageHeader
        title="Insights"
        description="How storage and cache growth are trending over time."
      />
      <p className="text-muted-foreground text-sm">
        Charts and cleanup history arrive once there's history worth charting — the scheduler has to
        record snapshots before a trend exists.
      </p>
    </div>
  );
}
