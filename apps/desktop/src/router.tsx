import { createHashRouter } from "react-router";

import App from "@/App";
import { DashboardPage } from "@/routes/dashboard";
import { DeveloperHealthPage } from "@/routes/developer-health";
import { InsightsPage } from "@/routes/insights";
import { NotFoundPage } from "@/routes/not-found";
import { SettingsPage } from "@/routes/settings";
import { SystemScanPage } from "@/routes/system-scan";

/**
 * Route table for the desktop app.
 *
 * Deliberately a **hash** router. In the packaged app the frontend is served by
 * Tauri's asset protocol, which has no server-side fallback to `index.html` —
 * so a hard reload or window recreation on a path like `/settings` can 404.
 * Everything after `#` never reaches that resolver, which sidesteps the problem
 * entirely. The cosmetic cost (`index.html#/settings`) is invisible in a desktop
 * window with no address bar. See docs/ARCHITECTURE.md, "Frontend".
 */
export const router = createHashRouter([
  {
    path: "/",
    element: <App />,
    children: [
      { index: true, element: <DashboardPage /> },
      { path: "developer-health", element: <DeveloperHealthPage /> },
      { path: "system-scan", element: <SystemScanPage /> },
      { path: "insights", element: <InsightsPage /> },
      { path: "settings", element: <SettingsPage /> },
      // Catch-all sits *inside* the layout's children on purpose. As a
      // top-level route it would render without the sidebar, leaving no way
      // back into the app short of editing the URL.
      { path: "*", element: <NotFoundPage /> },
    ],
  },
]);
