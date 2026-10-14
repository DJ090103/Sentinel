import { Flame } from "lucide-react";
import { useMemo } from "react";
import { Link } from "react-router";

import { CountUp } from "@/components/count-up";
import { PageHeader } from "@/components/page-header";
import { TrendChart } from "@/components/trend-chart";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { useInsights } from "@/hooks/use-insights";
import { cleanStreak } from "@/lib/insights";
import { formatBytes } from "@/lib/system-info";

const GIB = 1024 ** 3;

/**
 * What Sentinel has recovered and how the disk has moved.
 *
 * The reclaimed total is the one big number on the page — it is the only figure
 * here the user caused.
 */
export function InsightsPage() {
  const { data, error, isLoading } = useInsights();

  const trend = useMemo(
    () =>
      (data?.snapshots ?? []).flatMap((s) =>
        s.diskUsedBytes === null ? [] : [{ at: Date.parse(s.takenAt), used: s.diskUsedBytes }],
      ),
    [data],
  );

  if (error !== null) {
    return (
      <div className="space-y-6">
        <PageHeader
          title="Insights"
          description="What you've reclaimed, and how your disk is trending."
        />
        <div
          role="alert"
          className="border-destructive/50 bg-destructive/10 text-destructive rounded-2xl border-2 px-4 py-3 text-sm"
        >
          <p className="font-medium">Couldn&rsquo;t load your history</p>
          <p className="mt-1 break-words opacity-90">{error}</p>
        </div>
      </div>
    );
  }

  if (isLoading || data === null) {
    return (
      <div className="space-y-6" aria-busy="true">
        <PageHeader
          title="Insights"
          description="What you've reclaimed, and how your disk is trending."
        />
        <div className="bg-muted h-48 animate-pulse rounded-3xl" />
      </div>
    );
  }

  const streak = cleanStreak(data.cleanups);
  const total = data.totalReclaimedBytes;

  return (
    <div className="space-y-8">
      <section className="bg-card relative overflow-hidden rounded-[2rem] border-2 p-8 md:p-10">
        <div
          aria-hidden="true"
          className="bg-success/15 pointer-events-none absolute -bottom-24 -left-16 size-72 rounded-full blur-3xl"
        />
        <div className="relative space-y-3">
          <p className="text-muted-foreground text-base font-semibold">You&rsquo;ve reclaimed</p>
          <h1 className="font-display text-6xl leading-none font-extrabold tracking-tight md:text-7xl">
            {total === 0 ? (
              "0 GiB"
            ) : (
              <CountUp
                value={total / GIB}
                format={(n) => (n >= 10 ? n.toFixed(0) : n.toFixed(2)) + " GiB"}
              />
            )}
          </h1>
          <div className="flex flex-wrap items-center gap-3 pt-2">
            <span className="bg-secondary inline-flex items-center gap-2 rounded-full px-4 py-2 text-sm font-bold">
              <Flame aria-hidden="true" className={streak > 0 ? "text-primary size-5" : "size-5"} />
              {streak > 0
                ? `${streak}-day cleaning streak`
                : "No streak yet. Clean something today to start one."}
            </span>
            <span className="text-muted-foreground text-sm">
              {data.cleanCount} {data.cleanCount === 1 ? "clean" : "cleans"} so far
            </span>
          </div>
        </div>
      </section>

      <Card>
        <CardContent className="space-y-4">
          <h2 className="font-display text-2xl font-extrabold tracking-tight">
            Disk usage, last 30 days
          </h2>
          {trend.length >= 2 ? (
            <TrendChart points={trend} />
          ) : (
            <p className="text-muted-foreground max-w-prose text-sm">
              Sentinel records a reading in the background while it&rsquo;s open. A trend appears
              after a couple of readings, so check back soon.
            </p>
          )}
        </CardContent>
      </Card>

      <section className="space-y-4">
        <h2 className="font-display text-2xl font-extrabold tracking-tight">Cleanup history</h2>
        {data.cleanups.length === 0 ? (
          <Card>
            <CardContent className="space-y-4">
              <p className="text-muted-foreground max-w-prose text-sm">
                Nothing cleaned yet. Scan your machine, review what Sentinel finds, and your first
                clean will show up here.
              </p>
              <Button asChild>
                <Link to="/system-scan">Scan this machine</Link>
              </Button>
            </CardContent>
          </Card>
        ) : (
          <ol className="space-y-3">
            {data.cleanups.map((c) => (
              <li
                key={`${c.cleanedAt}-${c.pluginId}`}
                className="bg-card flex items-center justify-between gap-4 rounded-2xl border-2 px-5 py-4"
              >
                <div className="min-w-0">
                  <p className="truncate font-bold">{c.removed.join(", ") || c.pluginId}</p>
                  <p className="text-muted-foreground text-sm">
                    {new Date(c.cleanedAt).toLocaleString()}
                    {c.failureCount > 0 && ` · ${c.failureCount} couldn't be removed`}
                  </p>
                </div>
                <p className="font-display text-success shrink-0 text-xl font-extrabold">
                  {formatBytes(c.reclaimedBytes)}
                </p>
              </li>
            ))}
          </ol>
        )}
      </section>
    </div>
  );
}
