import { Channel, invoke } from "@tauri-apps/api/core";
import { useCallback, useState } from "react";

import type { AnalysisProgress, HomeAnalysis } from "@/lib/analysis";

/** Runs the read-only home-folder analysis when asked, never on its own. */
export function useHomeAnalysis() {
  const [analysis, setAnalysis] = useState<HomeAnalysis | null>(null);
  const [progress, setProgress] = useState<AnalysisProgress | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(async () => {
    setIsRunning(true);
    setError(null);
    setProgress(null);

    const channel = new Channel<AnalysisProgress>();
    channel.onmessage = setProgress;

    try {
      setAnalysis(await invoke<HomeAnalysis>("analyze_home", { onProgress: channel }));
    } catch (cause) {
      // A user-requested cancel is not a failure worth a banner.
      if (String(cause) !== "cancelled") {
        setError(String(cause));
      }
    } finally {
      setIsRunning(false);
    }
  }, []);

  const cancel = useCallback(() => {
    void invoke("cancel_analysis");
  }, []);

  return { analysis, progress, isRunning, error, run, cancel };
}
