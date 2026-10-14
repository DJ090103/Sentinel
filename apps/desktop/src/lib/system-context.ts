import { createContext } from "react";

import type { SystemInfo } from "@/lib/system-info";

/** One CPU/memory reading kept for the live charts. */
export interface HistoryPoint {
  cpu: number;
  mem: number;
}

/** How many readings the charts keep: 150 at 2s each is five minutes. */
export const HISTORY_LENGTH = 150;

/** What every page can read about the machine, polled once for the whole app. */
export interface SystemContextValue {
  data: SystemInfo | null;
  error: string | null;
  isLoading: boolean;
  history: HistoryPoint[];
}

export const SystemContext = createContext<SystemContextValue | null>(null);
