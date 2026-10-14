/**
 * Fixture backend for running the UI in a plain browser (`pnpm dev`), where
 * there is no Tauri. Loaded only in dev and only when Tauri is absent, so it
 * never ships in the app. It exists so the interface can be screenshotted and
 * reviewed without the Rust side.
 */
import { mockIPC } from "@tauri-apps/api/mocks";

const GIB = 1024 ** 3;

export function installDevMock() {
  mockIPC((cmd, args) => {
    switch (cmd) {
      case "get_system_info":
        return {
          host: {
            hostname: "dev-box",
            osName: "Windows 11",
            osVersion: "23H2",
            kernelVersion: "10.0.22631",
            uptimeSecs: 9 * 86400,
          },
          cpu: { usagePercent: 23.4, coreCount: 12 },
          memory: { totalBytes: 16 * GIB, usedBytes: 9.2 * GIB, availableBytes: 6.8 * GIB },
          disk: { mountPoint: "C:\\", totalBytes: 512 * GIB, availableBytes: 118 * GIB },
        };
      case "get_insights": {
        const now = Date.now();
        return {
          snapshots: Array.from({ length: 30 }, (_, i) => ({
            takenAt: new Date(now - (29 - i) * 86_400_000).toISOString(),
            cpuPercent: 20,
            ramUsedBytes: 8 * GIB,
            ramTotalBytes: 16 * GIB,
            diskUsedBytes: (380 + i * 1.1 - (i > 20 ? 18 : 0)) * GIB,
            diskTotalBytes: 512 * GIB,
          })),
          cleanups: [
            {
              cleanedAt: new Date(now).toISOString(),
              pluginId: "npm",
              removed: ["npm cache"],
              failureCount: 0,
              reclaimedBytes: 3.4 * GIB,
            },
            {
              cleanedAt: new Date(now - 86_400_000).toISOString(),
              pluginId: "junk",
              removed: ["Thumbnail cache (normal)", "Temp files"],
              failureCount: 1,
              reclaimedBytes: 1.2 * GIB,
            },
          ],
          totalReclaimedBytes: 18.6 * GIB,
          cleanCount: 7,
        };
      }
      case "get_companion_settings":
        return {
          enabled: true,
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
          avatar: { body: "#e6dfcc", eyes: "#7dffd0", screen: "#0c2426", shape: "classic" },
          x: null,
          y: null,
        };
      case "get_usage":
        return [
          { day: "2026-10-14", activity: "dev", app: "code", seconds: 14000 },
          { day: "2026-10-14", activity: "terminal", app: "alacritty", seconds: 5000 },
          { day: "2026-10-14", activity: "browser", app: "firefox", seconds: 4000 },
          { day: "2026-10-14", activity: "video", app: "vlc", seconds: 1500 },
        ];
      case "get_last_scan_at":
        return new Date().toISOString();
      case "scan_plugins":
        return { scanId: 1, report: { plugins: [] } };
      case "get_recommendations":
        return [];
      case "analyze_home": {
        const channel = (args as { onProgress?: { onmessage?: (m: unknown) => void } }).onProgress;
        channel?.onmessage?.({ stage: "walking", files: 48000, bytes: 30 * GIB });
        return {
          folders: [
            { path: "C:\\Users\\dev\\Downloads", sizeBytes: 41 * GIB },
            { path: "C:\\Users\\dev\\Videos", sizeBytes: 28 * GIB },
            { path: "C:\\Users\\dev\\source", sizeBytes: 19 * GIB },
          ],
          largestFiles: [{ path: "C:\\Users\\dev\\Videos\\render.mp4", sizeBytes: 6 * GIB }],
          duplicates: [
            {
              sizeBytes: 2 * GIB,
              paths: ["C:\\Users\\dev\\Downloads\\iso.zip", "C:\\Users\\dev\\Desktop\\iso.zip"],
            },
          ],
          totalFiles: 48210,
          totalBytes: 92 * GIB,
          truncated: false,
        };
      }
      default:
        return null;
    }
  });
}
