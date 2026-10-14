import { CompanionSettingsCard } from "@/components/companion-settings";
import { ModeToggle } from "@/components/mode-toggle";
import { PageHeader } from "@/components/page-header";
import { Card, CardContent } from "@/components/ui/card";

/** Application preferences: how Sentinel looks, and the desktop companion. */
export function SettingsPage() {
  return (
    <div className="space-y-8">
      <PageHeader title="Settings" description="Make Sentinel look and behave the way you like." />

      <Card>
        <CardContent className="space-y-3">
          <div className="space-y-1">
            <h2 className="font-display text-2xl font-extrabold tracking-tight">Appearance</h2>
            <p className="text-muted-foreground text-sm">
              &ldquo;System&rdquo; follows your operating system&rsquo;s light or dark setting.
            </p>
          </div>
          <ModeToggle />
        </CardContent>
      </Card>

      <CompanionSettingsCard />
    </div>
  );
}
