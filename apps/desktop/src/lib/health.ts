import type { HistoryPoint } from "@/lib/system-context";
import type { SystemInfo } from "@/lib/system-info";
import { usedPercent } from "@/lib/system-info";

export type HealthLevel = "good" | "warn" | "bad";

export interface HealthPoint {
  level: HealthLevel;
  text: string;
}

export interface Health {
  level: HealthLevel;
  headline: string;
  points: HealthPoint[];
}

const ORDER: HealthLevel[] = ["bad", "warn", "good"];

const average = (values: number[]) =>
  values.length === 0 ? 0 : values.reduce((a, b) => a + b, 0) / values.length;

/**
 * Turns readings into a plain-English verdict. Same thresholds as the
 * companion app so both agree about what "busy" means. Only uses what the
 * backend reports: CPU, memory, disk and uptime.
 */
export function summarizeHealth(info: SystemInfo, history: HistoryPoint[]): Health {
  const points: HealthPoint[] = [];
  const add = (level: HealthLevel, text: string) => points.push({ level, text });

  const cpu = average(history.slice(-20).map((p) => p.cpu));
  const cpuText = `${Math.round(cpu)}%`;
  add(
    cpu > 85 ? "bad" : cpu > 60 ? "warn" : "good",
    cpu > 85
      ? `CPU is pinned at ${cpuText}`
      : cpu > 60
        ? `CPU is busy (${cpuText})`
        : `CPU is relaxed (${cpuText})`,
  );

  const mem = usedPercent(info.memory.usedBytes, info.memory.totalBytes);
  const memText = `${Math.round(mem)}%`;
  add(
    mem > 95 ? "bad" : mem > 85 ? "warn" : "good",
    mem > 95
      ? `Memory is almost full (${memText})`
      : mem > 85
        ? `Memory is getting tight (${memText})`
        : `Memory is healthy (${memText})`,
  );

  if (info.disk) {
    const used = usedPercent(info.disk.totalBytes - info.disk.availableBytes, info.disk.totalBytes);
    const diskText = `${Math.round(used)}%`;
    add(
      used > 95 ? "bad" : used > 85 ? "warn" : "good",
      used > 95
        ? `Disk is almost full (${diskText}). Run a scan to take space back`
        : used > 85
          ? `Disk is filling up (${diskText})`
          : `Disk has room (${diskText} used)`,
    );
  }

  const days = Math.floor(info.host.uptimeSecs / 86400);
  if (days >= 14) {
    add("warn", `Up for ${days} days. A restart would freshen things up`);
  }

  const level = ORDER.find((l) => points.some((p) => p.level === l)) ?? "good";
  return {
    level,
    headline:
      level === "bad"
        ? "Your system is overloaded"
        : level === "warn"
          ? "Your system is a little stressed"
          : "Your system is running fine",
    points: points.sort((a, b) => ORDER.indexOf(a.level) - ORDER.indexOf(b.level)),
  };
}
