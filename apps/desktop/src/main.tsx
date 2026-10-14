import { MotionConfig } from "framer-motion";
import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "react-router";
import { ThemeProvider } from "@/components/theme-provider";
import { router } from "@/router";
import "./index.css";

// Browser-only fixtures for design review; stripped from production builds.
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  const { installDevMock } = await import("./dev-mock");
  installDevMock();
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider>
      <MotionConfig reducedMotion="user">
        <RouterProvider router={router} />
      </MotionConfig>
    </ThemeProvider>
  </React.StrictMode>,
);
