import { memo, useMemo, type CSSProperties } from "react";
import { DEFAULT_AVATAR } from "./defaults";
import type { AvatarStyle, Shape } from "./types";
import type { Face, Fx } from "./face";
import { H, W } from "./dims";
import { CHEEK, EYES, GLASSES, MOUTHS, sprite, type SpriteName } from "./sprites";

const SX = 6; // screen origin on the canvas
const SY = 16;

const COLOR: Record<string, string> = {
  "#": "var(--fg)",
  w: "#f4fff8",
  p: "var(--pink)",
  b: "#6fc3ff",
  d: "var(--dark)",
  r: "#ff4d5a",
  o: "#ff9a3c",
  y: "#ffe36e",
  g: "#c9d3dc",
  c: "#bff3ff",
};

const cache = new Map<string, Record<string, string>>();
/** Rows of chars -> one merged SVG path per colour char. */
function paths(rows: string[], mirror = false): Record<string, string> {
  const key = rows.join("/") + mirror;
  const hit = cache.get(key);
  if (hit) return hit;
  const out: Record<string, string> = {};
  rows.forEach((row0, y) => {
    const row = mirror ? [...row0].reverse().join("") : row0;
    for (let x = 0; x < row.length;) {
      const c = row[x]!;
      let n = 1;
      while (row[x + n] === c) n++;
      if (c !== ".") out[c] = (out[c] ?? "") + `M${x} ${y}h${n}v1h${-n}z`;
      x += n;
    }
  });
  cache.set(key, out);
  return out;
}

interface SpriteProps {
  rows: string[];
  x?: number;
  y?: number;
  mirror?: boolean;
  opacity?: number;
  fill?: string; // force a single colour
}
export const Sprite = memo(({ rows, x = 0, y = 0, mirror, opacity, fill }: SpriteProps) => (
  <g transform={`translate(${x} ${y})`} opacity={opacity}>
    {Object.entries(paths(rows, mirror)).map(([c, d]) => (
      <path key={c} d={d} style={{ fill: fill ?? COLOR[c] }} />
    ))}
  </g>
));

const R = ({
  x,
  y,
  w,
  h,
  c,
  o,
}: {
  x: number;
  y: number;
  w: number;
  h: number;
  c: string;
  o?: number;
}) => <rect x={x} y={y} width={w} height={h} style={{ fill: c }} opacity={o} />;

/** Animated, positioned effect sprites. Each `<g>` animates via CSS keyframes in styles.css. */
function Effect({ fx, face }: { fx: Fx; face: Face }) {
  const S = (n: SpriteName, x: number, y: number, cls: string, d = 0, extra?: CSSProperties) => (
    <g className={cls} style={{ animationDelay: `${d}s`, ...extra }}>
      <Sprite rows={sprite(n)} x={x} y={y} />
    </g>
  );
  switch (fx) {
    case "hearts":
      return (
        <>
          {S("heart", 8, 12, "rise", 0)}
          {S("heart", 17, 10, "rise", 0.5)}
          {S("heart", 27, 12, "rise", 1)}
        </>
      );
    case "sweat":
      return (
        <>
          {S("drop", 31, 15, "drip", 0)}
          {S("drop", 3, 20, "drip", 0.7)}
        </>
      );
    case "heat":
      return (
        <g style={{ opacity: 0.35 + face.heat * 0.65 }}>
          {S("wave", 11, 8, "waft", 0)}
          {S("wave", 19, 6, "waft", 0.4)}
          {S("wave", 27, 8, "waft", 0.8)}
        </g>
      );
    case "steam":
      return (
        <>
          {S("puff", 9, 12, "puff", 0)}
          {S("puff", 30, 12, "puff", 0.6)}
          {S("puff", 20, 12, "puff", 1.1)}
        </>
      );
    case "sparkles":
      return (
        <>
          {S("spark", 0, 9, "twinkle", 0)}
          {S("spark", 34, 6, "twinkle", 0.4)}
          {S("spark", 17, 2, "twinkle", 0.8)}
        </>
      );
    case "zzz":
      return (
        <>
          {S("z", 30, 11, "zz", 0)}
          {S("z", 33, 7, "zz", 0.6)}
          {S("z", 35, 3, "zz", 1.2)}
        </>
      );
    case "bubble":
      return S(face.state === "ERROR" ? "bang" : "dots", 27, 5, "bubble");
    case "anger":
      return S("anger", 30, 8, "throb");
    case "wind":
      return (
        <>
          {S("wind", 0, 20, "gust", 0)}
          {S("wind", 0, 25, "gust", 0.5)}
          {S("wind", 0, 30, "gust", 1)}
        </>
      );
    case "notes":
      return (
        <>
          {S("note", 4, 10, "rise", 0)}
          {S("note", 31, 8, "rise", 0.8)}
        </>
      );
    case "flame":
      return S("flame", 17, 3, "flicker");
    case "glitch":
      return (
        <g className="glitch" transform={`translate(${SX} ${SY})`}>
          <R x={0} y={4} w={28} h={1} c="var(--fg)" o={0.5} />
          <R x={0} y={11} w={28} h={2} c="#ff4d5a" o={0.35} />
        </g>
      );
  }
}

const HEAD = [
  [3, 12, 34, 2],
  [2, 14, 2, 5],
  [34, 14, 2, 5],
  [0, 19, 4, 9],
  [36, 19, 4, 9],
] as const;

const lerp = (a: number[], b: number[], t: number) =>
  a.map((v, i) => Math.round(v + (b[i]! - v) * t));
const rgb = (c: number[]) => `rgb(${c.join(",")})`;

const hexRgb = (h: string) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));

function screenBg(s: number, on: number[], glow: number[]) {
  const off = on.map((v) => Math.round(v * 0.4));
  const flash = lerp(on, glow, 0.45);
  return rgb(s <= 1 ? lerp(off, on, s) : lerp(on, flash, Math.min(1, (s - 1) / 0.6)));
}

type Rect4 = readonly [number, number, number, number];
interface Silhouette {
  body: Rect4[];
  bezel: Rect4[];
  hiTop: Rect4;
  hiLeft: Rect4;
  shade: Rect4;
  side: Rect4;
}
const CLASSIC: Silhouette = {
  body: [
    [6, 14, 28, 23],
    [5, 15, 30, 21],
    [4, 16, 32, 19],
  ],
  bezel: [[5, 15, 30, 19]],
  hiTop: [6, 14, 28, 1],
  hiLeft: [4, 16, 1, 17],
  shade: [5, 36, 30, 1],
  side: [35, 17, 1, 18],
};
const BOXY: Silhouette = {
  body: [[4, 14, 32, 23]],
  bezel: [[5, 15, 30, 19]],
  hiTop: [4, 14, 32, 1],
  hiLeft: [4, 15, 1, 19],
  shade: [4, 36, 32, 1],
  side: [35, 16, 1, 20],
};
const SHAPE: Record<Shape, Silhouette> = {
  classic: CLASSIC,
  cat: CLASSIC,
  boxy: BOXY,
  tv: BOXY,
  rounded: {
    body: [
      [8, 14, 24, 23],
      [6, 15, 28, 21],
      [5, 16, 30, 19],
      [4, 18, 32, 15],
    ],
    bezel: [
      [6, 15, 28, 19],
      [5, 16, 30, 17],
    ],
    hiTop: [8, 14, 24, 1],
    hiLeft: [4, 18, 1, 13],
    shade: [8, 36, 24, 1],
    side: [35, 19, 1, 13],
  },
};

const EAR_L: Rect4[] = [
  [6, 13, 4, 1],
  [6, 12, 3, 1],
  [6, 11, 2, 1],
  [6, 10, 1, 1],
];
const flipX = ([x, y, w, h]: Rect4): Rect4 => [40 - x - w, y, w, h];
const ANT_L: Rect4[] = [
  [16, 12, 1, 1],
  [15, 11, 1, 1],
  [14, 10, 1, 1],
  [13, 9, 1, 1],
  [12, 8, 1, 1],
  [11, 7, 1, 1],
  [10, 6, 2, 2],
];

/** Extras above the head that make each shape recognisable. */
function Decor({ shape }: { shape: Shape }) {
  if (shape === "cat")
    return (
      <>
        {[...EAR_L, ...EAR_L.map(flipX)].map((r, i) => (
          <R key={i} x={r[0]} y={r[1]} w={r[2]} h={r[3]} c="var(--body)" />
        ))}
        <R x={7} y={12} w={1} h={1} c="var(--pink)" />
        <R x={32} y={12} w={1} h={1} c="var(--pink)" />
      </>
    );
  if (shape === "tv")
    return (
      <>
        <R x={16} y={13} w={8} h={1} c="#3d4a4f" />
        {[...ANT_L, ...ANT_L.map(flipX)].map((r, i) => (
          <R key={i} x={r[0]} y={r[1]} w={r[2]} h={r[3]} c="#3d4a4f" />
        ))}
        <R x={12} y={35} w={2} h={1} c="var(--body-lo)" />
        <R x={15} y={35} w={1} h={1} c="var(--body-lo)" />
      </>
    );
  return null;
}

const LED: Record<string, string> = {
  ERROR: "#ff4d5a",
  OVERLOADED: "#ff4d5a",
  COOLING: "#ff9a3c",
  BUSY: "#ff9a3c",
};

interface Props {
  face: Face;
  scale: number;
  look?: [number, number];
  pulse?: number;
  avatar?: AvatarStyle;
  /** Embed as a nested <svg> at scene coordinates (x, y, width) instead of sizing in CSS px. */
  at?: { x: number; y: number; w: number };
}

export const Monitor = memo(
  ({ face, scale, look = [0, 0], pulse = 0, avatar = DEFAULT_AVATAR, at }: Props) => {
    const { eyes, mouth, cheeks } = face;
    const style = useMemo(
      () =>
        ({
          "--fg": `color-mix(in srgb, #ff4d5a ${Math.round(face.heat * 100)}%, ${avatar.eyes})`,
          "--body": avatar.body,
          "--body-hi": `color-mix(in srgb, ${avatar.body}, white 55%)`,
          "--body-lo": `color-mix(in srgb, ${avatar.body}, black 20%)`,
          "--body-lo2": `color-mix(in srgb, ${avatar.body}, black 10%)`,
          "--pink":
            cheeks === "red"
              ? `color-mix(in srgb, #ff3b3b ${Math.round(face.heat * 100)}%, #ff8fb1)`
              : "#ff8fb1",
          "--dark": "#0a1517",
          ...(at ? {} : { width: W * scale, height: H * scale }),
          filter: `drop-shadow(0 0 ${scale}px rgba(125,255,208,${0.12 * face.screen}))`,
        }) as CSSProperties,
      [face.heat, face.screen, cheeks, scale, at, avatar.eyes, avatar.body],
    );
    const on = face.screen > 0.05;
    const sil = SHAPE[avatar.shape];
    const screenRgb = hexRgb(avatar.screen);
    const glowRgb = hexRgb(avatar.eyes);
    const cheekOpacity =
      cheeks === "light" ? 0.75 : cheeks === "strong" ? 0.95 : 0.55 + face.heat * 0.45;
    const motion =
      face.state === "OVERLOADED"
        ? "shake"
        : face.state === "SLEEPING"
          ? "snooze"
          : face.state === "IDLE" || face.state === "WORKING"
            ? "bob"
            : "";

    return (
      <svg
        viewBox={`0 0 ${W} ${H}`}
        {...(at && { x: at.x, y: at.y, width: at.w, height: (at.w * H) / W })}
        style={style}
        shapeRendering="crispEdges"
        className={`monitor ${face.state}`}
      >
        <g className={motion}>
          <g key={pulse} className={pulse ? "pop" : ""}>
            {/* stand */}
            <R x={18} y={37} w={4} h={3} c="var(--body-lo)" />
            <R x={13} y={40} w={14} h={2} c="var(--body-lo2)" />
            <R x={12} y={41} w={16} h={1} c="var(--body-lo)" />
            <R x={12} y={42} w={16} h={1} c="#000" o={0.18} />
            {/* headphones sit behind the body */}
            {face.accessories.includes("headphones") && (
              <g>
                {HEAD.map(([x, y, w, h], i) => (
                  <R key={i} x={x} y={y} w={w} h={h} c={i > 2 ? "#8b7cff" : "#6a5cd6"} />
                ))}
                <R x={1} y={21} w={1} h={5} c="#c8c0ff" />
                <R x={38} y={21} w={1} h={5} c="#c8c0ff" />
              </g>
            )}
            <Decor shape={avatar.shape} />
            {/* body */}
            {sil.body.map((r, i) => (
              <R key={i} x={r[0]} y={r[1]} w={r[2]} h={r[3]} c="var(--body)" />
            ))}
            <R
              x={sil.hiTop[0]}
              y={sil.hiTop[1]}
              w={sil.hiTop[2]}
              h={sil.hiTop[3]}
              c="var(--body-hi)"
            />
            <R
              x={sil.hiLeft[0]}
              y={sil.hiLeft[1]}
              w={sil.hiLeft[2]}
              h={sil.hiLeft[3]}
              c="var(--body-hi)"
            />
            <R
              x={sil.shade[0]}
              y={sil.shade[1]}
              w={sil.shade[2]}
              h={sil.shade[3]}
              c="var(--body-lo)"
            />
            <R
              x={sil.side[0]}
              y={sil.side[1]}
              w={sil.side[2]}
              h={sil.side[3]}
              c="var(--body-lo2)"
            />
            {/* bezel + screen */}
            {sil.bezel.map((r, i) => (
              <R key={i} x={r[0]} y={r[1]} w={r[2]} h={r[3]} c="#26393e" />
            ))}
            <R x={SX} y={SY} w={28} h={17} c={screenBg(face.screen, screenRgb, glowRgb)} />
            {/* chin details */}
            <R x={17} y={35} w={6} h={1} c="var(--body-lo)" />
            <R x={30} y={35} w={2} h={1} c={on ? (LED[face.state] ?? "#5dff8a") : "#5a5a4a"} />

            {/* face */}
            <g transform={`translate(${SX} ${SY})`}>
              <g opacity={face.alpha} className="face">
                <g transform={`translate(${look[0]} ${look[1]})`}>
                  {eyes !== "none" && (
                    <>
                      <Sprite
                        rows={EYES[eyes]}
                        x={5}
                        y={3}
                        fill={eyes === "red" ? "#ff4d5a" : undefined}
                      />
                      <Sprite
                        rows={EYES[eyes]}
                        x={18}
                        y={3}
                        mirror
                        fill={eyes === "red" ? "#ff4d5a" : undefined}
                      />
                    </>
                  )}
                  {mouth !== "none" && <Sprite rows={MOUTHS[mouth]} x={10} y={10} />}
                </g>
                {cheeks !== "none" && (
                  <>
                    <Sprite rows={CHEEK[cheeks]} x={1} y={10} opacity={cheekOpacity} />
                    <Sprite rows={CHEEK[cheeks]} x={23} y={10} opacity={cheekOpacity} />
                  </>
                )}
                {(face.accessories.includes("glasses") ||
                  face.accessories.includes("workGlasses")) && (
                  <g transform="translate(0 2)">
                    <Sprite rows={GLASSES} fill="#f4fff8" opacity={0.9} />
                    {face.accessories.includes("workGlasses") && (
                      <>
                        <R x={5} y={1} w={5} h={6} c="#6fc3ff" o={0.22} />
                        <R x={18} y={1} w={5} h={6} c="#6fc3ff" o={0.22} />
                      </>
                    )}
                  </g>
                )}
              </g>
              {/* scanlines + glass glare */}
              {on &&
                Array.from({ length: 8 }, (_, i) => (
                  <R key={i} x={0} y={i * 2 + 1} w={28} h={1} c="#000" o={0.14} />
                ))}
              <R x={1} y={1} w={5} h={1} c="#fff" o={0.16 * face.screen} />
              <R x={1} y={2} w={1} h={3} c="#fff" o={0.16 * face.screen} />
            </g>
          </g>
          {face.alpha > 0 && face.effects.map((fx) => <Effect key={fx} fx={fx} face={face} />)}
        </g>
      </svg>
    );
  },
);
