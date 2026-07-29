import { useContext } from "react";

import { ThemeContext, type ThemeContextValue } from "@/lib/theme-context";

/**
 * Reads the current theme preference and the setter for it.
 *
 * Thin by design — it exists so components import a hook rather than the
 * context object directly, which keeps the context free to change shape.
 */
export function useTheme(): ThemeContextValue {
  return useContext(ThemeContext);
}
