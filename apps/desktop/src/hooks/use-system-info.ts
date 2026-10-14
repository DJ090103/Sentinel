import { useContext } from "react";

import { SystemContext, type SystemContextValue } from "@/lib/system-context";

/**
 * How often the backend is asked for a fresh reading.
 *
 * The floor is `MINIMUM_REFRESH_INTERVAL` in `crates/core/src/system`
 * (200ms): CPU usage is a delta between two samples, so polling faster returns
 * 0, not fresher data. Two seconds is above the floor and slow enough to read.
 */
export const SYSTEM_INFO_POLL_MS = 2000;

/** The app-wide system reading and its recent history. */
export function useSystemInfo(): SystemContextValue {
  const value = useContext(SystemContext);
  if (value === null) {
    throw new Error("useSystemInfo must be used inside <SystemProvider>");
  }
  return value;
}
