import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from "react";

import { ThemeContext } from "@/lib/theme-context";
import {
  applyResolvedTheme,
  getSystemPrefersDark,
  readStoredTheme,
  resolveTheme,
  storeTheme,
  watchSystemTheme,
  type Theme,
} from "@/lib/theme";

/**
 * Owns the app's theme preference and keeps the DOM in sync with it.
 *
 * The stored preference is read during `useState` initialisation rather than in
 * an effect, so the very first render already agrees with the class the inline
 * script in `index.html` set. Reading it in an effect would paint one frame of
 * the wrong theme before correcting itself.
 *
 * The OS preference is *subscribed to*, not copied into state, so there is no
 * second source of truth to keep in sync. `resolvedTheme` is derived on every
 * render instead — cheap, and impossible to leave stale.
 */
export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(readStoredTheme);

  // Subscribed unconditionally, which is fine: when the user has pinned light
  // or dark, `resolveTheme` simply ignores this value. The alternative —
  // subscribing only while `theme === "system"` — buys nothing and adds a
  // resubscribe on every preference change.
  const systemPrefersDark = useSyncExternalStore(watchSystemTheme, getSystemPrefersDark);

  const resolvedTheme = resolveTheme(theme, systemPrefersDark);

  useEffect(() => {
    applyResolvedTheme(resolvedTheme);
  }, [resolvedTheme]);

  const setTheme = useCallback((next: Theme) => {
    storeTheme(next);
    setThemeState(next);
  }, []);

  const value = useMemo(
    () => ({ theme, resolvedTheme, setTheme }),
    [theme, resolvedTheme, setTheme],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}
