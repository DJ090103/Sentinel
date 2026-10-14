import { Card, CardContent } from "@/components/ui/card";
import { useUsage } from "@/hooks/use-usage";
import { ACTIVITY_LABEL, buildProfile, formatDuration } from "@/lib/profile";

const BAR_COLORS = ["bg-primary", "bg-success", "bg-destructive", "bg-ring", "bg-muted-foreground"];

/**
 * "What kind of user are you", from the last 7 days of foreground-app use.
 * Needs the companion's app detection (Settings), so it explains that when
 * there is nothing recorded yet.
 */
export function ProfileCard() {
  const { rows, error } = useUsage();
  if (error !== null || rows === null) {
    return null;
  }
  const profile = buildProfile(rows);

  return (
    <Card>
      <CardContent className="space-y-4">
        <div className="space-y-1">
          <h2 className="font-display text-2xl font-extrabold tracking-tight">{profile.title}</h2>
          <p className="text-muted-foreground max-w-prose text-sm">{profile.blurb}</p>
        </div>
        {profile.shares.length > 0 ? (
          <>
            <div
              className="flex h-3 w-full overflow-hidden rounded-full"
              role="img"
              aria-label="Share of active time by activity"
            >
              {profile.shares.slice(0, 5).map(([activity, share], i) => (
                <div
                  key={activity}
                  className={BAR_COLORS[i]}
                  style={{ width: `${Math.max(2, share * 100)}%` }}
                />
              ))}
            </div>
            <ul className="grid gap-x-6 gap-y-1 text-sm sm:grid-cols-2">
              {profile.shares.slice(0, 5).map(([activity, share], i) => (
                <li key={activity} className="flex items-center gap-2">
                  <span aria-hidden="true" className={`${BAR_COLORS[i]} size-2.5 rounded-full`} />
                  <span className="font-bold">{ACTIVITY_LABEL[activity] ?? activity}</span>
                  <span className="text-muted-foreground">{Math.round(share * 100)}%</span>
                </li>
              ))}
            </ul>
            {profile.topApps.length > 0 && (
              <p className="text-muted-foreground text-sm">
                Most used:{" "}
                {profile.topApps.map(([a, s]) => `${a} (${formatDuration(s)})`).join(", ")}
              </p>
            )}
          </>
        ) : (
          <p className="text-muted-foreground text-sm">
            Nothing recorded yet. Sentinel notes which kind of app is in front while you work. Names
            and categories only, never window contents. You can turn this off in Settings.
          </p>
        )}
      </CardContent>
    </Card>
  );
}
