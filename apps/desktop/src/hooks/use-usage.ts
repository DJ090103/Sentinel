import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import type { UsageRow } from "@/companion/types";

/** The last 7 days of recorded app usage; `null` rows means still loading. */
export function useUsage() {
  const [rows, setRows] = useState<UsageRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<UsageRow[]>("get_usage", { days: 7 })
      .then((r) => !cancelled && setRows(r))
      .catch((cause) => !cancelled && setError(String(cause)));
    return () => {
      cancelled = true;
    };
  }, []);

  return { rows, error };
}
