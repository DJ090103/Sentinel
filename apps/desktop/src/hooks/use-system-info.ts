import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import type { SystemInfo } from "@/lib/system-info";

/**
 * How often the dashboard asks the backend for a fresh reading.
 *
 * The floor is `MINIMUM_REFRESH_INTERVAL` in `crates/core/src/system`
 * (200ms on every supported platform): CPU usage is a delta between two
 * samples, so polling faster than that returns 0, not fresher data. Two seconds
 * is comfortably above the floor and slow enough that the numbers are readable
 * rather than flickering.
 */
export const SYSTEM_INFO_POLL_MS = 2000;

/** What the Dashboard needs in order to render all three states honestly. */
export interface UseSystemInfoResult {
  /** Latest reading, or `null` before the first one arrives. */
  data: SystemInfo | null;
  /** Message from the most recent failed poll, or `null`. */
  error: string | null;
  /** True only before the first result — a refresh isn't a load. */
  isLoading: boolean;
}

/**
 * Polls the backend for system readings.
 *
 * A previous successful reading is deliberately kept on screen when a later
 * poll fails, with the error shown alongside it. Blanking the dashboard because
 * one poll in a two-second loop failed would be worse than briefly-stale
 * numbers, and the error makes the staleness visible rather than silent.
 */
export function useSystemInfo(intervalMs: number = SYSTEM_INFO_POLL_MS): UseSystemInfoResult {
  const [data, setData] = useState<SystemInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // Guards against a poll that resolves after the component unmounted or the
    // interval changed, which would otherwise set state on a dead subscription.
    let cancelled = false;

    async function poll() {
      try {
        const next = await invoke<SystemInfo>("get_system_info");
        if (!cancelled) {
          setData(next);
          setError(null);
        }
      } catch (cause) {
        if (!cancelled) {
          setError(cause instanceof Error ? cause.message : String(cause));
        }
      }
    }

    void poll();
    const timer = setInterval(() => void poll(), intervalMs);

    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [intervalMs]);

  return { data, error, isLoading: data === null && error === null };
}
