import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";

import type { CleanOutcome, CleanRequestError, ScanSnapshot, ScanSurface } from "@/lib/plugins";

/** What the Developer Health page needs to render every state honestly. */
export interface UsePluginScanResult {
  /** Latest scan, or `null` before the first one completes. */
  snapshot: ScanSnapshot | null;
  /** True while a scan is running. */
  isScanning: boolean;
  /** Message from a failed scan, or `null`. */
  error: string | null;
  /** Runs a fresh scan. The previous scan's positions stop being valid. */
  rescan: () => void;
  /** Removes the given positions from the current scan. */
  clean: (pluginId: string, indices: number[]) => Promise<CleanOutcome>;
}

/** A `clean_targets` rejection, carried so the page can react to its shape. */
export class CleanRejected extends Error {
  constructor(readonly reason: CleanRequestError) {
    super(reason.kind);
    this.name = "CleanRejected";
  }
}

/** How a page wants its scanning driven. */
export interface UsePluginScanOptions {
  /** Which page's plugins to scan. */
  surface: ScanSurface;
  /**
   * Whether to scan as soon as the page opens. Defaults to `true`.
   *
   * See the hook's docs — the answer is not the same for both pages.
   */
  scanOnMount?: boolean;
}

/**
 * Scans one surface's plugins, and cleans what the user picked.
 *
 * # Whether to scan on mount depends on what the page scans
 *
 * Developer Health scans on open, and should: its entire purpose is showing
 * what your tools are costing, it only stats a handful of known cache
 * directories, and an empty screen with a "Scan" button makes the user ask for
 * something they already asked for by opening the page.
 *
 * That reasoning does not carry to System Scan. Walking the user's own folders
 * — Trash, screenshots — is work they should ask for explicitly, not something
 * that starts because they clicked a sidebar item. Hence `scanOnMount`.
 *
 * There is no polling and no interval, unlike `useSystemInfo`. A scan walks
 * hundreds of thousands of files, and nothing here changes unless the user acts.
 *
 * Rescanning is driven by a counter rather than by calling a shared function,
 * so the work stays inside the effect that owns its cleanup — the same shape as
 * `useSystemInfo`.
 */
export function usePluginScan({
  surface,
  scanOnMount = true,
}: UsePluginScanOptions): UsePluginScanResult {
  const [snapshot, setSnapshot] = useState<ScanSnapshot | null>(null);
  const [isScanning, setIsScanning] = useState(scanOnMount);
  const [error, setError] = useState<string | null>(null);
  const [requestedScans, setRequestedScans] = useState(0);

  useEffect(() => {
    // Nothing has been asked for yet on a page that waits for a button.
    if (!scanOnMount && requestedScans === 0) {
      return;
    }

    // Guards against a scan resolving after unmount, and against an earlier
    // scan overwriting a later one when the user rescans mid-flight.
    let cancelled = false;

    async function run() {
      try {
        const next = await invoke<ScanSnapshot>("scan_plugins", { surface });
        if (!cancelled) {
          setSnapshot(next);
        }
      } catch (cause) {
        if (!cancelled) {
          setError(cause instanceof Error ? cause.message : String(cause));
        }
      } finally {
        if (!cancelled) {
          setIsScanning(false);
        }
      }
    }

    void run();

    return () => {
      cancelled = true;
    };
  }, [requestedScans, scanOnMount, surface]);

  const rescan = useCallback(() => {
    setIsScanning(true);
    setError(null);
    setRequestedScans((count) => count + 1);
  }, []);

  const clean = useCallback(
    async (pluginId: string, indices: number[]): Promise<CleanOutcome> => {
      if (snapshot === null) {
        throw new CleanRejected({ kind: "noScan" });
      }

      try {
        return await invoke<CleanOutcome>("clean_targets", {
          surface,
          scanId: snapshot.scanId,
          pluginId,
          targetIndices: indices,
        });
      } catch (cause) {
        // Tauri passes a command's `Err` value through as-is, so this is the
        // serialized `CleanRequestError` rather than an `Error`.
        if (typeof cause === "object" && cause !== null && "kind" in cause) {
          throw new CleanRejected(cause as CleanRequestError);
        }
        throw new CleanRejected({ kind: "failed", detail: String(cause) });
      }
    },
    [snapshot, surface],
  );

  return { snapshot, isScanning, error, rescan, clean };
}
