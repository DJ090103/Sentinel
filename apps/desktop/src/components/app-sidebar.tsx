import {
  Activity,
  HardDrive,
  LayoutDashboard,
  Settings,
  TrendingUp,
  type LucideIcon,
} from "lucide-react";
import { NavLink } from "react-router";

import { buttonVariants } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface NavItem {
  to: string;
  label: string;
  Icon: LucideIcon;
  /** Only for the index route: without it, "/" matches every path. */
  end?: boolean;
}

const NAV_ITEMS: NavItem[] = [
  { to: "/", label: "Dashboard", Icon: LayoutDashboard, end: true },
  { to: "/developer-health", label: "Developer Health", Icon: Activity },
  { to: "/system-scan", label: "System Scan", Icon: HardDrive },
  { to: "/insights", label: "Insights", Icon: TrendingUp },
  { to: "/settings", label: "Settings", Icon: Settings },
];

/**
 * Primary navigation.
 *
 * Uses `NavLink` rather than a button with an `onClick` handler so each entry is
 * a real link — keyboard and screen-reader users get link semantics, and
 * `NavLink` supplies `aria-current="page"` for the active route for free.
 *
 * Styled with `buttonVariants` instead of `Button`: these need to render as
 * anchors, and borrowing the class strings is exactly what that export is for.
 */
export function AppSidebar() {
  return (
    <aside className="bg-card flex w-60 shrink-0 flex-col border-r">
      <div className="flex items-center gap-3 border-b px-5 py-4">
        {/*
          Served from `public/`, so the same file backs this and the favicon in
          index.html — one copy, and they can't drift apart.

          `alt=""` and `aria-hidden`: the product name sits right beside it in
          text, so announcing the mark as well would just repeat it. Decorative
          here, not informative.
        */}
        <img src="/logo.png" alt="" aria-hidden="true" className="size-8 shrink-0" />
        <div className="min-w-0">
          <p className="font-semibold tracking-tight">Sentinel</p>
          <p className="text-muted-foreground text-xs">Workstation companion</p>
        </div>
      </div>

      <nav aria-label="Main" className="flex-1 space-y-1 p-3">
        {NAV_ITEMS.map(({ to, label, Icon, end }) => (
          <NavLink
            key={to}
            to={to}
            end={end}
            className={({ isActive }) =>
              cn(
                buttonVariants({ variant: isActive ? "secondary" : "ghost", size: "sm" }),
                "w-full justify-start gap-2",
              )
            }
          >
            <Icon aria-hidden="true" />
            {label}
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}
