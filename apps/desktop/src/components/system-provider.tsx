import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, type ReactNode } from "react";

import {
  HISTORY_LENGTH,
  SystemContext,
  type HistoryPoint,
  type SystemContextValue,
} from "@/lib/system-context";
import { SYSTEM_INFO_POLL_MS } from "@/hooks/use-system-info";
import { usedPercent, type SystemInfo } from "@/lib/system-info";

/**
 * Polls the backend once for the whole app and keeps a short history.
 *
 * Lives above the router so the charts keep their history while you move
 * between pages; a poll per page would restart the graph on every visit. The
 * previous reading stays on screen when a poll fails, with the error alongside.
 */
export function SystemProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<SystemContextValue>({
    data: null,
    error: null,
    isLoading: true,
    history: [],
  });

  useEffect(() => {
    let cancelled = false;

    async function poll() {
      try {
        const next = await invoke<SystemInfo>("get_system_info");
        if (cancelled) {
          return;
        }
        const point: HistoryPoint = {
          cpu: next.cpu.usagePercent,
          mem: usedPercent(next.memory.usedBytes, next.memory.totalBytes),
        };
        setState((prev) => ({
          data: next,
          error: null,
          isLoading: false,
          history: [...prev.history, point].slice(-HISTORY_LENGTH),
        }));
      } catch (cause) {
        if (!cancelled) {
          const error = cause instanceof Error ? cause.message : String(cause);
          setState((prev) => ({ ...prev, error, isLoading: false }));
        }
      }
    }

    void poll();
    const timer = setInterval(() => void poll(), SYSTEM_INFO_POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, []);

  return <SystemContext.Provider value={state}>{children}</SystemContext.Provider>;
}
