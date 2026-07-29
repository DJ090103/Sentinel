import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";

import { APP_EVENT_NAME, failureDetail, type AppEvent } from "@/lib/events";

/** What the Dashboard needs to describe the background scheduler honestly. */
export interface UseLastScanResult {
  /** RFC 3339 time of the last *successful* run, or `null` if there hasn't been one. */
  lastScanAt: string | null;
  /** Why the most recent run failed, or `null` if it succeeded. */
  failure: string | null;
  /** True only until the initial value has been fetched. */
  isLoading: boolean;
  /** Message from a failed initial fetch, or `null`. */
  error: string | null;
}

/**
 * Tracks when the background scheduler last ran.
 *
 * Two sources, deliberately:
 *
 * - One fetch at mount, because the event feed is live-only and the scheduler's
 *   first tick fires before the webview has subscribed. Without it the
 *   dashboard would claim nothing had ever been scanned for a full interval
 *   after every launch.
 * - The event stream thereafter. No polling: the backend already knows when a
 *   run finishes, and asking it repeatedly would be strictly worse information
 *   arriving strictly later.
 */
export function useLastScan(): UseLastScanResult {
  const [lastScanAt, setLastScanAt] = useState<string | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // A run can complete while the initial fetch is still in flight. Without this
  // the older fetched value would land second and overwrite the newer one.
  const hasLiveUpdate = useRef(false);

  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | undefined;

    async function subscribe() {
      const stop = await listen<AppEvent>(APP_EVENT_NAME, ({ payload }) => {
        if (payload.type !== "jobRan") {
          return;
        }

        hasLiveUpdate.current = true;
        const detail = failureDetail(payload.run.outcome);
        setFailure(detail);

        // Only a successful run means the machine was actually read. Advancing
        // the timestamp on a failure would claim a scan that didn't happen.
        if (detail === null) {
          setLastScanAt(payload.run.finishedAt);
        }
      });

      // The effect can be torn down between `listen` being called and resolving.
      if (cancelled) {
        stop();
      } else {
        unlisten = stop;
      }
    }

    async function fetchInitial() {
      try {
        const recorded = await invoke<string | null>("get_last_scan_at");
        if (!cancelled && !hasLiveUpdate.current) {
          setLastScanAt(recorded);
        }
      } catch (cause) {
        if (!cancelled) {
          setError(cause instanceof Error ? cause.message : String(cause));
        }
      } finally {
        if (!cancelled) {
          setIsLoading(false);
        }
      }
    }

    // Subscribing first so a run completing during the fetch is still caught.
    void subscribe();
    void fetchInitial();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return { lastScanAt, failure, isLoading, error };
}
