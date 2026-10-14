import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { Outlet, useLocation } from "react-router";

import { SystemProvider } from "@/components/system-provider";
import { AppSidebar } from "@/components/app-sidebar";

/**
 * The app shell: persistent sidebar on the left, routed page on the right.
 *
 * Rendered as the router's root route, so the sidebar mounts once and survives
 * navigation rather than being torn down and rebuilt per page. The page itself
 * is keyed on the path so each navigation plays one short enter transition —
 * the only motion that happens without the user doing something.
 */
function App() {
  const { pathname } = useLocation();
  const reduce = useReducedMotion();

  return (
    <SystemProvider>
      <div className="bg-background text-foreground flex h-screen">
        <AppSidebar />
        <main className="flex-1 overflow-y-auto">
          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={pathname}
              className="mx-auto max-w-5xl p-8 md:p-10"
              initial={reduce ? false : { opacity: 0, y: 16, filter: "blur(4px)" }}
              animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
              exit={reduce ? undefined : { opacity: 0, y: -8, filter: "blur(4px)" }}
              transition={{ duration: 0.22, ease: [0.16, 1, 0.3, 1] }}
            >
              <Outlet />
            </motion.div>
          </AnimatePresence>
        </main>
      </div>
    </SystemProvider>
  );
}

export default App;
