import { useMemo } from "react";

import { initBrain } from "@/companion/brain";
import { faceFor } from "@/companion/face";
import { Monitor } from "@/companion/Monitor";
import { SHAPES, type CompanionSettings } from "@/companion/types";
import { Card, CardContent } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { useCompanionSettings } from "@/hooks/use-companion-settings";
import { cn } from "@/lib/utils";

const SHAPE_LABEL = {
  classic: "Classic",
  rounded: "Rounded",
  boxy: "Boxy",
  tv: "Retro TV",
  cat: "Cat",
} as const;

const SLEEP_OPTIONS: [number, string][] = [
  [60, "1 minute"],
  [180, "3 minutes"],
  [600, "10 minutes"],
  [1800, "30 minutes"],
];
const POLL_OPTIONS: [number, string][] = [
  [1000, "Every second"],
  [3000, "Every 3 seconds"],
  [5000, "Every 5 seconds"],
  [10000, "Every 10 seconds"],
];

/** One labelled setting: text on the left, its control on the right, a rule beneath. */
function Row({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-6 border-b py-3 last:border-b-0">
      <div className="min-w-0">
        <p className="font-bold">{label}</p>
        {hint && <p className="text-muted-foreground text-sm">{hint}</p>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="space-y-1">
      <h3 className="font-display text-xl font-extrabold tracking-tight">{title}</h3>
      <div>{children}</div>
    </section>
  );
}

function ColorField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <Row label={label}>
      <input
        type="color"
        value={value}
        aria-label={label}
        onChange={(e) => onChange(e.target.value)}
        className="h-10 w-16 cursor-pointer rounded-xl border-2 bg-transparent p-0.5"
      />
    </Row>
  );
}

function SelectField({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: number;
  options: [number, string][];
  onChange: (value: number) => void;
}) {
  // A hand-edited file can hold a value that isn't one of the presets.
  const list = options.some(([v]) => v === value)
    ? options
    : [...options, [value, `${value}`] as [number, string]];
  return (
    <Row label={label}>
      <select
        value={value}
        aria-label={label}
        onChange={(e) => onChange(Number(e.target.value))}
        className="bg-card text-foreground h-10 min-w-44 rounded-xl border-2 px-3 text-sm font-bold"
      >
        {list.map(([v, text]) => (
          <option key={v} value={v}>
            {text}
          </option>
        ))}
      </select>
    </Row>
  );
}

function Preview({ s }: { s: CompanionSettings }) {
  const face = useMemo(
    () => faceFor({ ...initBrain(0), phase: "running" }, 0, { blink: false, hover: false }),
    [],
  );
  return (
    <div className="bg-muted/50 flex items-end justify-center rounded-3xl p-6">
      <div className="text-center">
        <p className="mb-1 text-sm font-bold" style={{ color: s.avatar.eyes }}>
          {s.name}
        </p>
        <Monitor face={face} scale={Math.min(s.scale, 6) + 2} avatar={s.avatar} />
      </div>
    </div>
  );
}

/**
 * Everything about the floating companion: whether it shows, how it looks, and
 * when it reacts. Each change is saved at once through the backend.
 */
export function CompanionSettingsCard() {
  const { settings: s, error, update } = useCompanionSettings();

  if (s === null) {
    return error === null ? (
      <div className="bg-muted h-48 animate-pulse rounded-3xl" aria-busy="true" />
    ) : (
      <div
        role="alert"
        className="border-destructive/50 bg-destructive/10 text-destructive rounded-2xl border-2 px-4 py-3 text-sm"
      >
        <p className="font-medium">Couldn&rsquo;t load the companion settings</p>
        <p className="mt-1 break-words opacity-90">{error}</p>
      </div>
    );
  }

  const setAvatar = (patch: Partial<CompanionSettings["avatar"]>) =>
    update({ avatar: { ...s.avatar, ...patch } });
  const setThreshold = (key: keyof CompanionSettings["thresholds"], value: number) =>
    update({ thresholds: { ...s.thresholds, [key]: value } });

  return (
    <Card>
      <CardContent className="space-y-8">
        <div className="space-y-1">
          <h2 className="font-display text-2xl font-extrabold tracking-tight">Desktop companion</h2>
          <p className="text-muted-foreground max-w-prose text-sm">
            A small pixel monitor that sits on your desktop and reacts to how hard your machine is
            working.
          </p>
        </div>

        {error !== null && (
          <p role="alert" className="text-destructive text-sm">
            {error}
          </p>
        )}

        <div className="grid items-start gap-8 lg:grid-cols-[15rem_1fr]">
          <div className="space-y-4">
            <Preview s={s} />
            <Switch
              checked={s.enabled}
              onChange={(enabled) => update({ enabled })}
              label="Show on desktop"
              description="Opens in its own window you can drag anywhere."
            />
          </div>

          <div className="space-y-8">
            <Section title="Look">
              <Row label="Name">
                <input
                  key={s.name}
                  defaultValue={s.name}
                  maxLength={24}
                  aria-label="Companion name"
                  onBlur={(e) => e.target.value !== s.name && update({ name: e.target.value })}
                  className="bg-card text-foreground h-10 w-44 rounded-xl border-2 px-3 text-sm font-bold"
                />
              </Row>
              <Row label="Size" hint={`${s.scale}x`}>
                <input
                  type="range"
                  min={2}
                  max={12}
                  value={s.scale}
                  onChange={(e) => update({ scale: Number(e.target.value) })}
                  aria-label="Companion size"
                  className="accent-primary w-44"
                />
              </Row>
              <Row label="Shape">
                <div className="flex max-w-xs flex-wrap justify-end gap-2">
                  {SHAPES.map((shape) => (
                    <button
                      key={shape}
                      type="button"
                      aria-pressed={s.avatar.shape === shape}
                      onClick={() => setAvatar({ shape })}
                      className={cn(
                        "focus-visible:ring-ring h-9 rounded-xl border-2 px-3 text-sm font-bold outline-none focus-visible:ring-[3px]",
                        s.avatar.shape === shape &&
                          "bg-primary text-primary-foreground border-primary",
                      )}
                    >
                      {SHAPE_LABEL[shape]}
                    </button>
                  ))}
                </div>
              </Row>
              <ColorField
                label="Body"
                value={s.avatar.body}
                onChange={(body) => setAvatar({ body })}
              />
              <ColorField
                label="Eyes and glow"
                value={s.avatar.eyes}
                onChange={(eyes) => setAvatar({ eyes })}
              />
              <ColorField
                label="Screen"
                value={s.avatar.screen}
                onChange={(screen) => setAvatar({ screen })}
              />
            </Section>

            <Section title="Behavior">
              <Switch
                checked={s.alwaysOnTop}
                onChange={(alwaysOnTop) => update({ alwaysOnTop })}
                label="Stay on top of other windows"
              />
              <Switch
                checked={s.launchAtLogin}
                onChange={(launchAtLogin) => update({ launchAtLogin })}
                label="Start Sentinel when I log in"
              />
              <Switch
                checked={s.workMode}
                onChange={(workMode) => update({ workMode })}
                label="React to the app in front"
                description="Looks happy while you watch, focused while you code. Only the app's name and kind are noted, and the Dashboard profile is built from them. Not available on Wayland."
              />
              <Switch
                checked={s.readTitles}
                disabled={!s.workMode}
                onChange={(readTitles) => update({ readTitles })}
                label="Read browser tab titles"
                description="Tells YouTube from GitHub in a browser. Titles are matched in memory and never stored."
              />
              <SelectField
                label="Fall asleep after"
                value={s.sleepAfterSecs}
                options={SLEEP_OPTIONS}
                onChange={(sleepAfterSecs) => update({ sleepAfterSecs })}
              />
              <SelectField
                label="Check the machine"
                value={s.pollMs}
                options={POLL_OPTIONS}
                onChange={(pollMs) => update({ pollMs })}
              />
            </Section>

            <details className="group">
              <summary className="font-display cursor-pointer text-xl font-extrabold tracking-tight">
                When does it get stressed?
              </summary>
              <div className="pt-2">
                {(
                  [
                    ["cpuMedium", "Working: CPU %"],
                    ["cpuHigh", "Busy: CPU %"],
                    ["cpuVeryHigh", "Overloaded: CPU %"],
                    ["memHigh", "Busy: memory %"],
                    ["memVeryHigh", "Overloaded: memory %"],
                    ["appsHigh", "Busy: open apps"],
                    ["appsVeryHigh", "Overloaded: open apps"],
                  ] as const
                ).map(([key, label]) => (
                  <Row key={key} label={label}>
                    <input
                      type="number"
                      min={1}
                      value={s.thresholds[key]}
                      aria-label={label}
                      onChange={(e) => setThreshold(key, Number(e.target.value))}
                      className="bg-card text-foreground h-10 w-24 rounded-xl border-2 px-3 text-right font-bold"
                    />
                  </Row>
                ))}
              </div>
            </details>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
