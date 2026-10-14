import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";

import type { CompanionSettings } from "@/companion/types";

/**
 * Loads the companion settings and saves changes through the backend, which
 * validates them. The screen updates at once and is corrected by whatever the
 * backend returns.
 */
export function useCompanionSettings() {
  const [settings, setSettings] = useState<CompanionSettings | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<CompanionSettings>("get_companion_settings")
      .then((s) => !cancelled && setSettings(s))
      .catch((cause) => !cancelled && setError(String(cause)));
    return () => {
      cancelled = true;
    };
  }, []);

  const update = useCallback(
    (patch: Partial<CompanionSettings>) => {
      if (settings === null) return;
      const next = { ...settings, ...patch };
      setSettings(next);
      invoke<CompanionSettings>("set_companion_settings", { settings: next })
        .then((saved) => {
          setSettings(saved);
          setError(null);
        })
        .catch((cause) => setError(String(cause)));
    },
    [settings],
  );

  return { settings, error, update };
}
