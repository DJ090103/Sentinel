import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import type { Insights } from "@/lib/insights";

/** Loads stored history once when the page opens. */
export function useInsights() {
  const [data, setData] = useState<Insights | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<Insights>("get_insights")
      .then((result) => !cancelled && setData(result))
      .catch((cause) => !cancelled && setError(String(cause)));
    return () => {
      cancelled = true;
    };
  }, []);

  return { data, error, isLoading: data === null && error === null };
}
