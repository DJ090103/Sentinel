import { ModeToggle } from "@/components/mode-toggle";
import { PageHeader } from "@/components/page-header";

/**
 * Application preferences.
 *
 * Theme is the only real setting so far — it moved here from the placeholder
 * home page now that there's somewhere for it to belong. Phase 2 adds the rest
 * (scan interval, last-scan timestamp) and moves persistence off `localStorage`
 * onto the settings file in `crates/core/src/settings`.
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
