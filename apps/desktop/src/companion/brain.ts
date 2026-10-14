import type { Activity, CompanionSettings, Sample } from "./types";

export type AvatarState =
  | "IDLE"
  | "STARTING"
  | "WORKING"
  | "BUSY"
  | "OVERLOADED"
  | "BLUSHING"
  | "HAPPY"
  | "COOLING"
  | "SLEEPING"
  | "SURPRISED"
  | "ERROR"
  | "SHUTTING_DOWN";

export const BOOT_MS = 3200;
export const SHUTDOWN_MS = 1600;
const CLICK = { cooldown: 180, decay: 2200, react: 1800, max: 6 };
const SUSTAIN = 2; // samples a new load level must persist before we believe it

type Reaction = "SURPRISED" | "HAPPY" | "BLUSHING";

export interface Brain {
  phase: "starting" | "running" | "shutting";
  phaseAt: number;
  cpu: number;
  level: 0 | 1 | 2 | 3; // load: low, medium, high, very high
  cand: number;
  candN: number;
  byApps: boolean; // overload is (partly) caused by too many apps
  heat: number; // 0..1, rises fast, falls slowly => gradual cooling
  cooling: boolean;
  work: Activity | null;
  activity: Activity | null; // work or media, for the face's costume
  relaxed: boolean;
  sleeping: boolean;
  clickLevel: number;
  clickAt: number;
  decayAt: number;
  reaction: { state: Reaction; until: number } | null;
  error: string | null;
  lastTick: number;
}

export type Event =
  | { type: "tick" }
  | { type: "sample"; s: Sample }
  | { type: "click" }
  | { type: "error"; msg: string | null }
  | { type: "shutdown" };

export const initBrain = (now: number): Brain => ({
  phase: "starting",
  phaseAt: now,
  cpu: 0,
  level: 0,
  cand: 0,
  candN: 0,
  byApps: false,
  heat: 0,
  cooling: false,
  work: null,
  activity: null,
  relaxed: false,
  sleeping: false,
  clickLevel: 0,
  clickAt: 0,
  decayAt: 0,
  reaction: null,
  error: null,
  lastTick: now,
});

/** Map raw readings to a load level 0..3 using the configurable thresholds. */
export function classify(s: Sample, cpu: number, t: CompanionSettings["thresholds"]) {
  const c = cpu >= t.cpuVeryHigh ? 3 : cpu >= t.cpuHigh ? 2 : cpu >= t.cpuMedium ? 1 : 0;
  const m = s.mem >= t.memVeryHigh ? 3 : s.mem >= t.memHigh ? 2 : 0;
  const a = s.apps == null ? 0 : s.apps >= t.appsVeryHigh ? 3 : s.apps >= t.appsHigh ? 2 : 0;
  return { level: Math.max(c, m, a) as 0 | 1 | 2 | 3, byApps: a === 3 };
}

export function step(b: Brain, e: Event, now: number, cfg: CompanionSettings): Brain {
  if (b.phase === "shutting") return b;
  switch (e.type) {
    case "shutdown":
      return { ...b, phase: "shutting", phaseAt: now };

    case "error":
      return { ...b, error: e.msg };

    case "sample": {
      const cpu = e.s.cpu;
      const { level: cand, byApps } = classify(e.s, cpu, cfg.thresholds);
      let { level, candN } = b;
      if (cand === level) candN = 0;
      else {
        candN = cand === b.cand ? candN + 1 : 1;
        if (candN >= SUSTAIN) {
          level = cand;
          candN = 0;
        }
      }
      const sleeping = (e.s.idleSec ?? 0) >= cfg.sleepAfterSecs;
      const wake = b.sleeping && !sleeping;
      return {
        ...b,
        cpu,
        level,
        cand,
        candN,
        sleeping,
        cooling: level === 3 ? false : b.cooling || (b.level === 3 && b.heat > 0.05),
        byApps: level === 3 ? byApps || b.byApps : false,
        work: cfg.workMode ? e.s.work : null,
        activity: cfg.workMode ? (e.s.activity ?? e.s.work) : null,
        relaxed: !!e.s.relaxed,
        reaction: wake ? { state: "SURPRISED", until: now + 1200 } : b.reaction,
      };
    }

    case "click": {
      if (b.phase === "starting" || now - b.clickAt < CLICK.cooldown) return b;
      const clickLevel = Math.min(CLICK.max, b.clickLevel + 1);
      const state: Reaction =
        clickLevel === 1 ? "SURPRISED" : clickLevel === 2 ? "HAPPY" : "BLUSHING";
      const until = now + CLICK.react + (clickLevel >= 5 ? 1200 : 0);
      return {
        ...b,
        clickLevel,
        clickAt: now,
        decayAt: now,
        reaction: { state, until },
        sleeping: false,
      };
    }

    case "tick": {
      const dt = Math.min(1, (now - b.lastTick) / 1000);
      let n: Brain = { ...b, lastTick: now };
      if (b.phase === "starting" && now - b.phaseAt >= BOOT_MS)
        n = { ...n, phase: "running", phaseAt: now };

      const target = n.level === 3 ? 1 : 0;
      n.heat =
        n.heat < target
          ? Math.min(target, n.heat + dt * 0.35)
          : Math.max(target, n.heat - dt * 0.09);
      if (n.level === 3) n.cooling = false;
      if (n.cooling && n.heat <= 0.03) {
        n.cooling = false;
        n.reaction = { state: "HAPPY", until: now + 2000 }; // relief
      }
      if (n.reaction && now >= n.reaction.until) n.reaction = null;
      if (n.clickLevel > 0 && now - n.decayAt > CLICK.decay) {
        n.clickLevel -= 1;
        n.decayAt = now;
      }
      return n;
    }
  }
}

export function avatarState(b: Brain, now: number): AvatarState {
  if (b.phase === "shutting") return "SHUTTING_DOWN";
  if (b.phase === "starting") return "STARTING";
  if (b.error) return "ERROR";
  if (b.level === 3) return "OVERLOADED";
  if (b.cooling) return "COOLING";
  if (b.reaction && now < b.reaction.until) return b.reaction.state;
  if (b.level === 2) return "BUSY";
  if (b.sleeping) return "SLEEPING";
  if (b.work || b.level === 1) return "WORKING";
  if (b.relaxed) return "HAPPY";
  return "IDLE";
}
