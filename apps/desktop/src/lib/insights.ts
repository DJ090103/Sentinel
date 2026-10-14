/** TypeScript mirror of `sentinel_core::database::history`. Rust is the source of truth. */

export interface SnapshotPoint {
  takenAt: string;
  cpuPercent: number | null;
  ramUsedBytes: number;
  ramTotalBytes: number;
  diskUsedBytes: number | null;
  diskTotalBytes: number | null;
}

export interface CleanupRecord {
  cleanedAt: string;
  pluginId: string;
  removed: string[];
  failureCount: number;
  reclaimedBytes: number;
}

export interface Insights {
  snapshots: SnapshotPoint[];
  cleanups: CleanupRecord[];
  totalReclaimedBytes: number;
  cleanCount: number;
}

const dayKey = (date: Date) => `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;

/**
 * Consecutive local days, ending today or yesterday, on which something was
 * cleaned. Yesterday still counts so the streak isn't shown as lost before the
 * day is over.
 */
export function cleanStreak(cleanups: CleanupRecord[], now: Date = new Date()): number {
  const days = new Set(cleanups.map((c) => dayKey(new Date(c.cleanedAt))));
  const cursor = new Date(now);
  if (!days.has(dayKey(cursor))) {
    cursor.setDate(cursor.getDate() - 1);
  }
  let streak = 0;
  while (days.has(dayKey(cursor))) {
    streak += 1;
    cursor.setDate(cursor.getDate() - 1);
  }
  return streak;
}
