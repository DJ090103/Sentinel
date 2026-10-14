import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useSyncExternalStore } from "react";

import { initBrain, step, type Brain, type Event } from "./brain";
import { DEFAULT_COMPANION } from "./defaults";
import { faceFor, faceKey, type Face, type Ui } from "./face";
import {
  COMPANION_SETTINGS_EVENT,
  SAMPLE_EVENT,
  type AvatarStyle,
  type CompanionSettings,
  type Sample,
} from "./types";

interface View {
  face: Face;
  scale: number;
  look: [number, number];
  pulse: number;
  name: string;
  avatar: AvatarStyle;
}

// A tiny external store (what zustand would give us) so there is no new
// dependency for one object.
const now = () => performance.now();
let view: View = {
  face: faceFor(initBrain(0), 0, { blink: false, hover: false }),
  scale: DEFAULT_COMPANION.scale,
  look: [0, 0],
  pulse: 0,
  name: DEFAULT_COMPANION.name,
  avatar: DEFAULT_COMPANION.avatar,
};
const listeners = new Set<() => void>();
const setView = (patch: Partial<View>) => {
  view = { ...view, ...patch };
  listeners.forEach((l) => l());
};
const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => listeners.delete(l);
};
export const useAvatar = () => useSyncExternalStore(subscribe, () => view);

// The state machine lives outside React so StrictMode remounts can't replay the boot.
let brain: Brain = initBrain(now());
let cfg: CompanionSettings = DEFAULT_COMPANION;
const ui: Ui = { blink: false, hover: false };
let lastKey = "";
let started = false;

function refresh() {
  const face = faceFor(brain, now(), ui);
  const key = faceKey(face);
  if (key !== lastKey) {
    lastKey = key;
    setView({ face });
  }
}

function apply(e: Event) {
  brain = step(brain, e, now(), cfg);
  refresh();
}

/** A poke. Returns true if it registered (not swallowed by the cooldown). */
export function click() {
  const before = brain.clickAt;
  apply({ type: "click" });
  const registered = brain.clickAt !== before;
  if (registered) setView({ pulse: view.pulse + 1 });
  return registered;
}

export function setHover(h: boolean) {
  ui.hover = h;
  if (!h) setView({ look: [0, 0] });
  refresh();
}

export const setLook = (x: number, y: number) => setView({ look: [x, y] });

const rand = (a: number, b: number) => a + Math.random() * (b - a);

/** Starts the state machine and wires it to the backend's sample and settings events. */
export function start() {
  if (started) return;
  started = true;

  setInterval(() => apply({ type: "tick" }), 100);
  const blink = () => {
    ui.blink = true;
    refresh();
    setTimeout(() => {
      ui.blink = false;
      refresh();
      setTimeout(blink, rand(2500, 6000));
    }, 130);
  };
  setTimeout(blink, 3500);
  setInterval(() => {
    if (!ui.hover) setLook(Math.round(rand(-1, 1)), 0);
  }, 4200);

  const useSettings = (s: CompanionSettings) => {
    cfg = s;
    setView({ scale: s.scale, name: s.name, avatar: s.avatar });
    refresh();
  };

  void listen<Sample>(SAMPLE_EVENT, (e) => apply({ type: "sample", s: e.payload }));
  void listen<CompanionSettings>(COMPANION_SETTINGS_EVENT, (e) => useSettings(e.payload));
  void invoke<CompanionSettings>("get_companion_settings")
    .then((s) => {
      useSettings(s);
      brain = initBrain(now()); // the boot animation starts once the window is up
    })
    .catch(() => undefined);
}
