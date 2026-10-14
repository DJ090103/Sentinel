import { BOOT_MS, SHUTDOWN_MS, avatarState, type AvatarState, type Brain } from "./brain";

export type Eye =
  | "none"
  | "normal"
  | "happy"
  | "sleepy"
  | "closed"
  | "surprised"
  | "focused"
  | "angry"
  | "stressed"
  | "red"
  | "down"
  | "heart"
  | "crying"
  | "blink";
export type Mouth =
  | "none"
  | "neutral"
  | "smile"
  | "big"
  | "happy"
  | "sleepy"
  | "yawn"
  | "surprised"
  | "stressed"
  | "open"
  | "laughing";
export type Cheek = "none" | "light" | "strong" | "red";
export type Accessory = "glasses" | "workGlasses" | "headphones";
export type Fx =
  | "hearts"
  | "sweat"
  | "heat"
  | "steam"
  | "sparkles"
  | "zzz"
  | "bubble"
  | "anger"
  | "wind"
  | "flame"
  | "glitch"
  | "notes";

export interface Face {
  state: AvatarState;
  eyes: Eye;
  mouth: Mouth;
  cheeks: Cheek;
  accessories: Accessory[];
  effects: Fx[];
  heat: number; // 0..1 tint for cheeks/eyes
  alpha: number; // face opacity (fade in/out)
  screen: number; // 0 = off .. 1 = full glow
}

/** Transient, purely cosmetic inputs owned by the renderer. */
export interface Ui {
  blink: boolean;
  hover: boolean;
}

const base: Face = {
  state: "IDLE",
  eyes: "normal",
  mouth: "smile",
  cheeks: "none",
  accessories: [],
  effects: [],
  heat: 0,
  alpha: 1,
  screen: 1,
};

const gear = (b: Brain): Accessory[] =>
  b.work === "browser" || b.work === "office"
    ? ["workGlasses"]
    : b.work === "dev"
      ? ["workGlasses", "headphones"]
      : b.work === "terminal" || b.work === "game"
        ? ["headphones"]
        : [];

/** Costume for what the user is doing, on top of the working face. */
function activityFace(b: Brain): Partial<Face> | null {
  switch (b.work) {
    case "game":
      return { eyes: "focused", mouth: "open", effects: ["sparkles"] };
    case "chat":
      return { eyes: "normal", mouth: "happy", cheeks: "light", effects: ["bubble"] };
    case "design":
      return { eyes: "happy", mouth: "smile", effects: ["sparkles"] };
    case "office":
      return { eyes: "focused", mouth: "neutral", effects: [] };
    default:
      return null;
  }
}

function boot(ms: number): Partial<Face> {
  if (ms < 350) return { screen: 0, alpha: 0, eyes: "none", mouth: "none" };
  if (ms < 900) return { screen: 1.6, alpha: 0, eyes: "none", mouth: "none" }; // boot glow
  if (ms < 1500) return { screen: 1, alpha: (ms - 900) / 600, eyes: "closed", mouth: "none" };
  if (ms < 2300) return { eyes: "sleepy", mouth: "yawn", effects: ["zzz"] };
  if (ms < 2700) return { eyes: "sleepy", mouth: "sleepy" };
  return { eyes: "normal", mouth: "smile" };
}

function shutdown(ms: number): Partial<Face> {
  if (ms < 500) return { eyes: "sleepy", mouth: "smile" };
  if (ms < 1100) return { eyes: "closed", mouth: "sleepy" };
  return {
    eyes: "closed",
    mouth: "none",
    alpha: Math.max(0, 1 - (ms - 1100) / 300),
    screen: Math.max(0, 1 - ((ms - 1100) / SHUTDOWN_MS) * 2),
  };
}

export function faceFor(b: Brain, now: number, ui: Ui): Face {
  const state = avatarState(b, now);
  const f: Face = { ...base, state, heat: b.heat, accessories: [], effects: [] };
  const set = (p: Partial<Face>) => Object.assign(f, p);
  const t = now - b.phaseAt;

  switch (state) {
    case "STARTING":
      set(boot(Math.min(t, BOOT_MS)));
      break;
    case "SHUTTING_DOWN":
      set(shutdown(t));
      break;
    case "IDLE":
      set(ui.hover ? { mouth: "happy", cheeks: "light" } : {});
      break;
    case "WORKING":
      set({
        eyes: "focused",
        mouth: "neutral",
        accessories: gear(b),
        effects: b.work ? ["bubble"] : [],
        ...activityFace(b),
      });
      break;
    case "BUSY":
      set({ eyes: "focused", mouth: "stressed", effects: ["sweat"], accessories: gear(b) });
      break;
    case "OVERLOADED":
      set({
        eyes: b.heat > 0.45 ? "red" : "stressed",
        mouth: "stressed",
        cheeks: "red",
        effects: b.byApps ? ["sweat", "heat", "steam", "flame"] : ["sweat", "heat", "steam"],
      });
      break;
    case "COOLING":
      // hot -> sweating -> relieved: every step keyed off the decaying heat value
      if (b.heat > 0.65)
        set({ eyes: "stressed", mouth: "stressed", cheeks: "red", effects: ["sweat", "heat"] });
      else if (b.heat > 0.3)
        set({ eyes: "sleepy", mouth: "open", cheeks: "red", effects: ["sweat", "wind"] });
      else set({ eyes: "normal", mouth: "smile", cheeks: "light", effects: ["wind"] });
      break;
    case "SLEEPING":
      set({ eyes: "closed", mouth: "sleepy", effects: ["zzz"], screen: 0.55 });
      break;
    case "SURPRISED":
      set({ eyes: "surprised", mouth: "surprised" });
      break;
    case "HAPPY":
      if (b.relaxed && !(b.reaction && now < b.reaction.until))
        set(
          b.activity === "music"
            ? {
                eyes: "happy",
                mouth: "smile",
                cheeks: "light",
                accessories: ["headphones"],
                effects: ["notes"],
              }
            : { eyes: "normal", mouth: "smile", cheeks: "light", effects: ["sparkles"] },
        );
      else set({ eyes: "happy", mouth: "big", cheeks: "light", effects: ["sparkles"] });
      break;
    case "BLUSHING":
      if (b.clickLevel >= 5)
        set({ eyes: "angry", mouth: "stressed", cheeks: "strong", effects: ["anger"] });
      else if (b.clickLevel === 4)
        set({ eyes: "heart", mouth: "laughing", cheeks: "strong", effects: ["hearts"] });
      else set({ eyes: "down", mouth: "smile", cheeks: "strong", effects: ["hearts"] });
      break;
    case "ERROR":
      set({ eyes: "crying", mouth: "stressed", effects: ["bubble", "glitch"] });
      break;
  }
  if (ui.blink && (f.eyes === "normal" || f.eyes === "focused" || f.eyes === "happy"))
    f.eyes = "blink";
  return f;
}

/** Cheap identity so the store only re-renders when something visible changed. */
export const faceKey = (f: Face) =>
  [
    f.state,
    f.eyes,
    f.mouth,
    f.cheeks,
    f.accessories.join("+"),
    f.effects.join("+"),
    Math.round(f.heat * 20),
    Math.round(f.alpha * 10),
    Math.round(f.screen * 10),
  ].join("|");
