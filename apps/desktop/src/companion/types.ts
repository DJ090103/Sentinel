/**
 * TypeScript mirror of `sentinel_core::activity` and
 * `sentinel_core::settings::companion`. Rust is the source of truth.
 */

export type Activity =
  "browser" | "dev" | "terminal" | "video" | "game" | "music" | "chat" | "design" | "office";

export const SHAPES = ["classic", "rounded", "boxy", "tv", "cat"] as const;
export type Shape = (typeof SHAPES)[number];

export interface AvatarStyle {
  body: string;
  eyes: string;
  screen: string;
  shape: Shape;
}

export interface Thresholds {
  cpuMedium: number;
  cpuHigh: number;
  cpuVeryHigh: number;
  memHigh: number;
  memVeryHigh: number;
  appsHigh: number;
  appsVeryHigh: number;
}

export interface CompanionSettings {
  enabled: boolean;
  name: string;
  scale: number;
  alwaysOnTop: boolean;
  launchAtLogin: boolean;
  workMode: boolean;
  readTitles: boolean;
  sleepAfterSecs: number;
  pollMs: number;
  thresholds: Thresholds;
  avatar: AvatarStyle;
  x: number | null;
  y: number | null;
}

/** One sensor reading. `null` means the platform can't say; never a guess. */
export interface Sample {
  cpu: number;
  mem: number;
  apps: number | null;
  idleSec: number | null;
  work: Activity | null;
  relaxed: boolean;
  activity: Activity | null;
  app: string | null;
}

export interface UsageRow {
  day: string;
  activity: string;
  app: string;
  seconds: number;
}

export const SAMPLE_EVENT = "sentinel://sample";
export const COMPANION_SETTINGS_EVENT = "sentinel://companion-settings";
