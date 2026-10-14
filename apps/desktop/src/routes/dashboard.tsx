import { Clock, Cpu, HardDrive, MemoryStick, Monitor, Server } from "lucide-react";

import { Flame } from "lucide-react";

import { AreaChart } from "@/components/area-chart";
import { DiskHero } from "@/components/disk-hero";
import { PageHeader } from "@/components/page-header";
import { ProfileCard } from "@/components/profile-card";
import { StatCard, UnknownValue } from "@/components/stat-card";
import { Card, CardContent } from "@/components/ui/card";
import { useInsights } from "@/hooks/use-insights";
import { cleanStreak } from "@/lib/insights";
import { useLastScan } from "@/hooks/use-last-scan";
import { SYSTEM_INFO_POLL_MS, useSystemInfo } from "@/hooks/use-system-info";
import { formatScanTime } from "@/lib/events";
import { summarizeHealth, type HealthLevel } from "@/lib/health";
import { formatBytes, usedPercent, type SystemInfo } from "@/lib/system-info";

/**
 * When the background scheduler last checked the machine.
 *
 * Distinct from the cards above it, which are read live every couple of
 * seconds. This line is about the *background* work, and it changes only when
 * the backend says a run finished — never on a frontend timer.
 */
function LastScanNotice() {
  const { lastScanAt, failure, isLoading, error } = useLastScan();

  if (isLoading) {
    return <div className="bg-muted h-4 w-64 animate-pulse rounded" aria-hidden="true" />;
  }

  let message: string;
  if (error !== null) {
    message = "Couldn't read when the last background check ran.";
  } else if (lastScanAt === null) {
    message = "No background check has completed yet.";
  } else {
    const formatted = formatScanTime(lastScanAt);
    message =
      formatted === null
        ? "The last background check reported a time that couldn't be read."
        : `Last background check at ${formatted}.`;
  }

  return (
    <div
      role="status"
      className="text-muted-foreground flex flex-wrap items-center gap-x-2 gap-y-1 text-sm"
    >
      <Clock aria-hidden="true" className="size-4 shrink-0" />
      <span>{message}</span>
      {failure !== null && (
        <span className="text-destructive">The most recent one failed: {failure}</span>
      )}
    </div>
  );
}

/**
 * The reward line: what's been reclaimed and the streak, on the first screen.
 * Silent if history is unavailable; the Insights page explains why.
 */
function RewardStrip() {
  const { data } = useInsights();
  if (data === null || data.cleanCount === 0) {
    return null;
  }
  const streak = cleanStreak(data.cleanups);

  return (
    <div className="flex flex-wrap items-center gap-3">
      <span className="bg-secondary rounded-full px-4 py-2 text-sm font-bold">
        {formatBytes(data.totalReclaimedBytes)} reclaimed so far
      </span>
      {streak > 0 && (
        <span className="bg-secondary inline-flex items-center gap-2 rounded-full px-4 py-2 text-sm font-bold">
          <Flame aria-hidden="true" className="text-primary size-4" />
          {streak}-day streak
        </span>
      )}
    </div>
  );
}

const LEVEL_STYLE: Record<HealthLevel, string> = {
  good: "bg-success",
  warn: "bg-primary",
  bad: "bg-destructive",
};

/** Plain-English verdict plus the live CPU and memory charts. */
function HealthPanel({ info }: { info: SystemInfo }) {
  const { history } = useSystemInfo();
  const health = summarizeHealth(info, history);

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Card className="gap-3">
        <CardContent className="space-y-3">
          <h2 className="font-display text-2xl font-extrabold tracking-tight">{health.headline}</h2>
          <ul className="space-y-2">
            {health.points.map((p) => (
              <li key={p.text} className="flex items-start gap-3 text-sm">
                <span
                  aria-hidden="true"
                  className={`${LEVEL_STYLE[p.level]} mt-1.5 size-2.5 shrink-0 rounded-full`}
                />
                <span>{p.text}</span>
              </li>
            ))}
          </ul>
        </CardContent>
      </Card>

      <Card className="gap-3">
        <CardContent className="space-y-4">
          <div className="space-y-1">
            <p className="text-muted-foreground text-sm font-bold">CPU, last 5 minutes</p>
            <AreaChart data={history.map((p) => p.cpu)} color="var(--primary)" label="CPU" />
          </div>
          <div className="space-y-1">
            <p className="text-muted-foreground text-sm font-bold">Memory, last 5 minutes</p>
            <AreaChart data={history.map((p) => p.mem)} color="var(--success)" label="Memory" />
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

/** Skeleton shown only before the first reading arrives. */
function DashboardSkeleton() {
  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
      {Array.from({ length: 5 }, (_, index) => (
        <Card key={index} className="gap-3 py-4">
          <CardContent className="space-y-3 px-4">
            <div className="bg-muted h-3 w-20 animate-pulse rounded" />
            <div className="bg-muted h-6 w-32 animate-pulse rounded" />
          </CardContent>
        </Card>
      ))}
    </div>
  );
}

function SystemCards({ info }: { info: SystemInfo }) {
  const { host, cpu, memory, disk } = info;
  const memoryPercent = usedPercent(memory.usedBytes, memory.totalBytes);

  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
      <StatCard
        Icon={Cpu}
        label="CPU"
        value={`${cpu.usagePercent.toFixed(1)}%`}
        detail={`${cpu.coreCount} logical cores`}
        progressPercent={cpu.usagePercent}
      />

      <StatCard
        Icon={MemoryStick}
        label="Memory"
        value={formatBytes(memory.usedBytes)}
        detail={`of ${formatBytes(memory.totalBytes)} — ${formatBytes(memory.availableBytes)} available`}
        progressPercent={memoryPercent}
      />

      {disk ? (
        <StatCard
          Icon={HardDrive}
          label="Disk"
          value={formatBytes(disk.totalBytes - disk.availableBytes)}
          detail={`of ${formatBytes(disk.totalBytes)} — ${formatBytes(disk.availableBytes)} free, mounted at ${disk.mountPoint}`}
          progressPercent={usedPercent(disk.totalBytes - disk.availableBytes, disk.totalBytes)}
        />
      ) : (
        <StatCard
          Icon={HardDrive}
          label="Disk"
          value={<UnknownValue />}
          detail="No mounted filesystem reported"
        />
      )}

      <StatCard
        Icon={Monitor}
        label="Operating system"
        value={host.osName ?? <UnknownValue />}
        detail={host.kernelVersion ? `Kernel ${host.kernelVersion}` : "Kernel version not reported"}
      />

      <StatCard
        Icon={Server}
        label="Hostname"
        value={host.hostname ?? <UnknownValue />}
        detail={host.osVersion ? `Version ${host.osVersion}` : undefined}
      />
    </div>
  );
}

/**
 * Landing page — live system health at a glance.
 *
 * Values are read fresh from the OS on a short interval and never persisted;
 * see product-docs/ARCHITECTURE.md, "Persistence".
 */
export function DashboardPage() {
  const { data, error, isLoading } = useSystemInfo();

  return (
    <div className="space-y-6">
      {data?.disk ? (
        <DiskHero disk={data.disk} />
      ) : (
        <PageHeader
          title="Dashboard"
          description={`Live system health for this workstation, refreshed every ${SYSTEM_INFO_POLL_MS / 1000} seconds.`}
        />
      )}

      <RewardStrip />

      <LastScanNotice />

      {/* Shown above any existing data: a failed poll makes what's on screen
          stale, and hiding that would be worse than showing it. */}
      {error !== null && (
        <div
          role="alert"
          className="border-destructive/50 bg-destructive/10 text-destructive rounded-lg border px-4 py-3 text-sm"
        >
          <p className="font-medium">Couldn&rsquo;t read system information</p>
          <p className="mt-1 break-words opacity-90">{error}</p>
          {data !== null && (
            <p className="mt-1 opacity-90">Showing the last successful reading below.</p>
          )}
        </div>
      )}

      {isLoading && <DashboardSkeleton />}
      {data !== null && <HealthPanel info={data} />}
      {data !== null && <SystemCards info={data} />}
      <ProfileCard />
    </div>
  );
}
