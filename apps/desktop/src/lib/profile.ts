import type { UsageRow } from "@/companion/types";

/** What each tracked activity is called on screen. */
export const ACTIVITY_LABEL: Record<string, string> = {
  dev: "Coding",
  terminal: "Terminal",
  browser: "Browsing",
  video: "Watching",
  game: "Gaming",
  music: "Music",
  chat: "Chatting",
  design: "Creating",
  office: "Docs",
  other: "Other",
};

const PERSONAS = [
  {
    id: "nerd",
    title: "Certified nerd",
    blurb: "Editors, terminals and stack traces. You live in the code.",
    cats: ["dev", "terminal"],
  },
  { id: "movie", title: "Movie buff", blurb: "The screen is mostly showtime.", cats: ["video"] },
  {
    id: "gamer",
    title: "Gamer",
    blurb: "Frames per second matter more than seconds per task.",
    cats: ["game"],
  },
  {
    id: "social",
    title: "Social butterfly",
    blurb: "Chats, calls and inboxes keep you busy.",
    cats: ["chat"],
  },
  {
    id: "creator",
    title: "Creator",
    blurb: "Pixels, vertices and timelines: you make things.",
    cats: ["design"],
  },
  {
    id: "listener",
    title: "Music lover",
    blurb: "Something is always playing in the background.",
    cats: ["music"],
  },
  {
    id: "office",
    title: "Desk pro",
    blurb: "Documents and reading. Steady and organised.",
    cats: ["office"],
  },
  { id: "explorer", title: "Web explorer", blurb: "Tabs, tabs and more tabs.", cats: ["browser"] },
];

export interface Profile {
  title: string;
  blurb: string;
  totalSec: number;
  /** Share of active time per activity, biggest first. */
  shares: [string, number][];
  /** Most-used apps, biggest first. */
  topApps: [string, number][];
}

/** Who you are, judged from the usage rows (the last 7 days). */
export function buildProfile(rows: UsageRow[]): Profile {
  const cats: Record<string, number> = {};
  const apps: Record<string, number> = {};
  for (const r of rows) {
    cats[r.activity] = (cats[r.activity] ?? 0) + r.seconds;
    if (r.app) apps[r.app] = (apps[r.app] ?? 0) + r.seconds;
  }
  const total = Object.values(cats).reduce((a, b) => a + b, 0);
  const shares = Object.entries(cats)
    .map(([k, v]): [string, number] => [k, v / (total || 1)])
    .sort((a, b) => b[1] - a[1]);
  const topApps = Object.entries(apps)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5);

  if (total < 600) {
    return {
      title: "Getting to know you",
      blurb: "Use your computer for a bit and Sentinel will work out what kind of user you are.",
      totalSec: total,
      shares,
      topApps,
    };
  }
  let best = PERSONAS[0];
  let top = 0;
  for (const p of PERSONAS) {
    const share = p.cats.reduce((a, c) => a + (cats[c] ?? 0), 0) / total;
    if (share > top) {
      best = p;
      top = share;
    }
  }
  if (top < 0.25) {
    return {
      title: "All-rounder",
      blurb: "A bit of everything. No single habit defines you.",
      totalSec: total,
      shares,
      topApps,
    };
  }
  return { title: best.title, blurb: best.blurb, totalSec: total, shares, topApps };
}

/** "3h 20m" style duration. */
export function formatDuration(sec: number): string {
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}
