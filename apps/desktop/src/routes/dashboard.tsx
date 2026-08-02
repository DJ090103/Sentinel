import { Clock, Cpu, HardDrive, MemoryStick, Monitor, Server } from "lucide-react";

import { PageHeader } from "@/components/page-header";
import { StatCard, UnknownValue } from "@/components/stat-card";
import { Card, CardContent } from "@/components/ui/card";
import { useLastScan } from "@/hooks/use-last-scan";
import { SYSTEM_INFO_POLL_MS, useSystemInfo } from "@/hooks/use-system-info";
import { formatScanTime } from "@/lib/events";
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
 * see docs/ARCHITECTURE.md, "Persistence".
 */
export function DashboardPage() {
  const { data, error, isLoading } = useSystemInfo();

  return (
    <div className="space-y-6">
      <PageHeader
        title="Dashboard"
        description={`Live system health for this workstation, refreshed every ${SYSTEM_INFO_POLL_MS / 1000} seconds.`}
      />

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
      {data !== null && <SystemCards info={data} />}
    </div>
  );
}
