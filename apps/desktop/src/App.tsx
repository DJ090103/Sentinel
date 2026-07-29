import { Outlet } from "react-router";

import { AppSidebar } from "@/components/app-sidebar";

/**
 * The app shell: persistent sidebar on the left, routed page on the right.
 *
 * Rendered as the router's root route, so the sidebar mounts once and survives
 * navigation rather than being torn down and rebuilt per page.
 */
function App() {
  return (
    <div className="bg-background text-foreground flex h-screen">
      <AppSidebar />
      <main className="flex-1 overflow-y-auto p-8">
        <Outlet />
      </main>
    </div>
  );
}

export default App;
