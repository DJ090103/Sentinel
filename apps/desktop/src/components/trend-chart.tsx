import { motion } from "framer-motion";

import { formatBytes } from "@/lib/system-info";

const W = 640;
const H = 200;
const PAD = 8;

/**
 * Disk usage over time as a line with a soft fill. Hand-drawn SVG rather than a
 * chart library: it is one series, and this way it uses the theme's colors and
 * draws itself in without another dependency.
 */
export function TrendChart({ points }: { points: { at: number; used: number }[] }) {
  if (points.length < 2) {
    return null;
  }

  const xs = points.map((p) => p.at);
  const ys = points.map((p) => p.used);
  const [x0, x1] = [Math.min(...xs), Math.max(...xs)];
  const [lo, hi] = [Math.min(...ys), Math.max(...ys)];
  const spanY = hi - lo || 1;
  const spanX = x1 - x0 || 1;

  const coords = points.map((p) => [
    PAD + ((p.at - x0) / spanX) * (W - 2 * PAD),
    H - PAD - ((p.used - lo) / spanY) * (H - 2 * PAD),
  ]);
  const line = coords
    .map(([x, y], i) => `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`)
    .join(" ");
  const area = `${line} L${coords[coords.length - 1][0]},${H} L${coords[0][0]},${H} Z`;
  const change = ys[ys.length - 1] - ys[0];

  return (
    <figure className="space-y-3">
      <svg
        viewBox={`0 0 ${W} ${H}`}
        className="h-48 w-full"
        preserveAspectRatio="none"
        role="img"
        aria-label={`Disk used over time, ${change >= 0 ? "up" : "down"} ${formatBytes(Math.abs(change))}`}
      >
        <defs>
          <linearGradient id="trend-fill" x1="0" x2="0" y1="0" y2="1">
            <stop offset="0%" stopColor="var(--primary)" stopOpacity="0.35" />
            <stop offset="100%" stopColor="var(--primary)" stopOpacity="0" />
          </linearGradient>
        </defs>
        <motion.path
          d={area}
          fill="url(#trend-fill)"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ delay: 0.5, duration: 0.6 }}
        />
        <motion.path
          d={line}
          fill="none"
          stroke="var(--primary)"
          strokeWidth="4"
          strokeLinecap="round"
          strokeLinejoin="round"
          vectorEffect="non-scaling-stroke"
          initial={{ pathLength: 0 }}
          animate={{ pathLength: 1 }}
          transition={{ duration: 1.1, ease: [0.16, 1, 0.3, 1] }}
        />
      </svg>
      <figcaption className="text-muted-foreground text-sm">
        {change >= 0 ? "Grew" : "Shrank"} by {formatBytes(Math.abs(change))} over{" "}
        {Math.max(1, Math.round((x1 - x0) / 86_400_000))} days
      </figcaption>
    </figure>
  );
}
