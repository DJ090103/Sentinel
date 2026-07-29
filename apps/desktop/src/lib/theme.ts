/**
 * Theme preference model and the DOM/storage side effects behind it.
 *
 * These are plain functions rather than React code on purpose. The preference
 * has to be applied *before* React mounts (see the inline script in
 * `index.html`, which duplicates `THEME_STORAGE_KEY` and the resolution rule
 * below), and Phase 2 replaces `localStorage` here with the settings file in
 * `crates/core/src/settings`. Keeping the logic out of the component tree makes
 * that swap a change to two functions instead of a change to the provider.
 */

/** What the user chose. `system` defers to the OS rather than pinning a value. */
export type Theme = "light" | "dark" | "system";

/** What that choice actually resolves to right now — `system` collapsed away. */
export type ResolvedTheme = "light" | "dark";

/**
 * Storage key for the persisted preference.
 *
 * IMPORTANT: the inline script in `apps/desktop/index.html` hardcodes this same
 * string. It cannot import from here — it runs before the bundle loads. If you
 * change this key, change it there too, or a stored preference silently starts
 * flashing the wrong theme on cold start.
 */
export const THEME_STORAGE_KEY = "sentinel-ui-theme";

/** Follow the OS until the user says otherwise. */
export const DEFAULT_THEME: Theme = "system";

const DARK_MEDIA_QUERY = "(prefers-color-scheme: dark)";

function isTheme(value: unknown): value is Theme {
  return value === "light" || value === "dark" || value === "system";
}

/**
 * Reads the persisted preference, falling back to {@link DEFAULT_THEME}.
 *
 * Wrapped in try/catch because `localStorage` access throws outright when
 * storage is disabled or the quota is exhausted. A theme preference is not
 * worth crashing the app over — an unreadable preference is just an absent one.
 */
export function readStoredTheme(): Theme {
  try {
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    return isTheme(stored) ? stored : DEFAULT_THEME;
  } catch {
    return DEFAULT_THEME;
  }
}

/** Persists the preference. Silently no-ops if storage is unavailable. */
export function storeTheme(theme: Theme): void {
  try {
    localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    // Preference won't survive a restart, but the current session still works.
  }
}

/**
 * Subscribes to OS-level colour-scheme changes. Returns an unsubscribe fn.
 *
 * Shaped as a `useSyncExternalStore` subscriber (and kept at module scope so
 * the reference is stable across renders) because the OS preference is exactly
 * that: external state React has to read rather than own.
 */
export function watchSystemTheme(onChange: () => void): () => void {
  const query = window.matchMedia(DARK_MEDIA_QUERY);
  query.addEventListener("change", onChange);
  return () => query.removeEventListener("change", onChange);
}

/** Whether the OS currently asks for a dark colour scheme. */
export function getSystemPrefersDark(): boolean {
  return window.matchMedia(DARK_MEDIA_QUERY).matches;
}

/**
 * Collapses `system` into a concrete theme.
 *
 * Takes the OS preference as an argument rather than reading `matchMedia`
 * itself so it stays a pure function of its inputs — the caller already has
 * that value from the subscription, and re-reading it here would be a second
 * source of truth that can disagree mid-render.
 */
export function resolveTheme(theme: Theme, systemPrefersDark: boolean): ResolvedTheme {
  if (theme !== "system") {
    return theme;
  }
  return systemPrefersDark ? "dark" : "light";
}

/**
 * Applies a theme to the document by toggling the `dark` class on `<html>`.
 *
 * That class is what the `dark:` variant keys off — see the
 * `@custom-variant dark` declaration in `index.css`.
 */
export function applyResolvedTheme(resolved: ResolvedTheme): void {
  document.documentElement.classList.toggle("dark", resolved === "dark");
}
