import { createContext } from "react";

import { DEFAULT_THEME, type ResolvedTheme, type Theme } from "@/lib/theme";

/** What `useTheme()` hands back to consumers. */
export interface ThemeContextValue {
  /** The user's choice, which may be `system`. */
  theme: Theme;
  /** What `theme` currently resolves to — useful for picking an icon. */
  resolvedTheme: ResolvedTheme;
  /** Sets and persists the preference. */
  setTheme: (theme: Theme) => void;
}

/**
 * Lives in its own module, separate from `ThemeProvider` and `useTheme`.
 *
 * React Fast Refresh only works for a file whose exports are all components,
 * so the context object, the provider, and the hook are deliberately three
 * files. Co-locating them costs HMR on every edit to the provider.
 */
export const ThemeContext = createContext<ThemeContextValue>({
  theme: DEFAULT_THEME,
  resolvedTheme: "light",
  setTheme: () => {
    // Replaced by ThemeProvider. Standalone default keeps `useTheme` from
    // exploding if a component is rendered outside the provider in a test.
  },
});
