import { ModeToggle } from "@/components/mode-toggle";
import { PageHeader } from "@/components/page-header";

/**
 * Application preferences.
 *
 * Theme is the only preference exposed here. It currently persists in webview
 * `localStorage`; Rust has a separate settings file for scheduler state. See
 * `docs/ARCHITECTURE.md`, "Persistence", before changing either boundary.
 */
export function SettingsPage() {
  return (
    <div className="space-y-6">
      <PageHeader title="Settings" description="How Sentinel looks and behaves." />

      <section className="space-y-3">
        <div className="space-y-1">
          <h2 className="text-sm font-medium">Appearance</h2>
          <p className="text-muted-foreground text-sm">
            &ldquo;System&rdquo; follows your operating system&rsquo;s light or dark setting.
          </p>
        </div>
        <ModeToggle />
      </section>
    </div>
  );
}
