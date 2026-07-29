/**
 * TypeScript mirror of `SystemInfo` in `crates/core/src/system/mod.rs`.
 *
 * Hand-written rather than generated: one Tauri command and four small structs
 * don't justify a codegen step yet. The tradeoff is that these two definitions
 * can drift silently, so if you change the Rust structs, change these too — the
 * Rust side is the source of truth. Field names are camelCase because the core
 * types carry `#[serde(rename_all = "camelCase")]`.
 *
 * `null` is meaningful throughout: it means the OS genuinely didn't report the
 * value. Render it as unknown; don't substitute a plausible-looking default.
 */

/** Machine identity and OS. Every field can legitimately be absent. */
export interface HostInfo {
  hostname: string | null;
  osName: string | null;
  osVersion: string | null;
  kernelVersion: string | null;
}

/** Processor load at the moment of the snapshot. */
export interface CpuInfo {
  /** Average across all cores, 0–100. */
  usagePercent: number;
  /** Logical cores, including SMT/hyperthreads. */
  coreCount: number;
}

/** Physical memory, in bytes. */
export interface MemoryInfo {
  totalBytes: number;
  usedBytes: number;
  availableBytes: number;
}

/** The primary filesystem, in bytes. */
export interface DiskInfo {
  mountPoint: string;
  totalBytes: number;
  availableBytes: number;
}

/** One reading of the host machine. */
export interface SystemInfo {
  host: HostInfo;
  cpu: CpuInfo;
  memory: MemoryInfo;
  /** `null` when no mounted filesystem was found. */
  disk: DiskInfo | null;
}

/**
 * Formats a byte count using binary units (KiB/MiB/GiB).
 *
 * Binary rather than decimal because that's what the values are: `sysinfo`
 * reports what the kernel reports, and a "7.1 GiB" figure that matches `free`
 * and `df` is more useful than a "7.6 GB" figure that matches nothing the user
 * can cross-check.
 */
export function formatBytes(bytes: number): string {
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];

  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  // Whole bytes never need a decimal point; larger units read better with one.
  const decimals = unitIndex === 0 ? 0 : 1;
  return `${value.toFixed(decimals)} ${units[unitIndex]}`;
}

/** Percentage of a total that has been used, clamped to 0–100. */
export function usedPercent(used: number, total: number): number {
  if (total <= 0) {
    return 0;
  }
  return Math.min(100, Math.max(0, (used / total) * 100));
}
