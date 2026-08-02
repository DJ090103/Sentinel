import { Monitor, Moon, Sun, type LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useTheme } from "@/hooks/use-theme";
import type { Theme } from "@/lib/theme";

const OPTIONS: { value: Theme; label: string; Icon: LucideIcon }[] = [
  { value: "light", label: "Light", Icon: Sun },
  { value: "dark", label: "Dark", Icon: Moon },
  { value: "system", label: "System", Icon: Monitor },
];

/**
 * Theme switcher, rendered as three always-visible options.
 *
 * Deliberately not a single cycling button: with three states, a cycling
 * control gives no way to see which one is active or to reach a specific one
 * directly. `system` in particular is invisible in a two-state toggle, and it
 * is the default.
 *
 * Built from `Button` rather than a dropdown because three explicit choices
 * remain faster to inspect and use than a hidden menu.
 */
export function ModeToggle() {
  const { theme, setTheme } = useTheme();

  return (
    <div role="group" aria-label="Colour theme" className="flex items-center gap-1">
      {OPTIONS.map(({ value, label, Icon }) => {
        const isActive = theme === value;

        return (
          <Button
            key={value}
            type="button"
            size="sm"
            variant={isActive ? "secondary" : "ghost"}
            aria-pressed={isActive}
            onClick={() => setTheme(value)}
          >
            <Icon aria-hidden="true" />
            {label}
          </Button>
        );
      })}
    </div>
  );
}
