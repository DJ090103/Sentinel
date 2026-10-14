import type { AvatarStyle, CompanionSettings } from "./types";

export const DEFAULT_AVATAR: AvatarStyle = {
  body: "#e6dfcc",
  eyes: "#7dffd0",
  screen: "#0c2426",
  shape: "classic",
};

/** Mirrors `CompanionSettings::default()` in Rust; used until the backend answers. */
export const DEFAULT_COMPANION: CompanionSettings = {
  enabled: false,
  name: "Sentinel",
  scale: 3,
  alwaysOnTop: true,
  launchAtLogin: false,
  workMode: true,
  readTitles: true,
  sleepAfterSecs: 180,
  pollMs: 3000,
  thresholds: {
    cpuMedium: 35,
    cpuHigh: 65,
    cpuVeryHigh: 85,
    memHigh: 90,
    memVeryHigh: 97,
    appsHigh: 9,
    appsVeryHigh: 14,
  },
  avatar: DEFAULT_AVATAR,
  x: null,
  y: null,
};
