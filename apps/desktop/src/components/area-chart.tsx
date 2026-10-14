import { useId } from "react";

/**
 * A 0 to 100 line chart that fills as readings arrive. Hand-drawn SVG: it is
 * one series with fixed bounds, so a chart library would add weight for nothing.
 */
export function AreaChart({
  data,
  color,
  label,
}: {
  data: number[];
  color: string;
  label: string;
}) {
  const gradientId = useId();
  if (data.length < 2) {
    return <div className="bg-muted/40 h-24 animate-pulse rounded-2xl" aria-hidden="true" />;
  }
  const points = data.map(
    (v, i) => `${(i / (data.length - 1)) * 300},${100 - Math.min(100, Math.max(0, v))}`,
  );

  return (
    <svg
      viewBox="0 0 300 100"
      preserveAspectRatio="none"
      className="h-24 w-full"
      role="img"
      aria-label={`${label}: ${Math.round(data[data.length - 1])}% now`}
    >
      <defs>
        <linearGradient id={gradientId} x1="0" x2="0" y1="0" y2="1">
          <stop offset="0" stopColor={color} stopOpacity="0.4" />
          <stop offset="1" stopColor={color} stopOpacity="0" />
        </linearGradient>
      </defs>
      {[25, 50, 75].map((y) => (
        <line
          key={y}
          x1="0"
          x2="300"
          y1={y}
          y2={y}
          stroke="currentColor"
          strokeOpacity="0.1"
          vectorEffect="non-scaling-stroke"
        />
      ))}
      <polygon points={`0,100 ${points.join(" ")} 300,100`} fill={`url(#${gradientId})`} />
      <polyline
        points={points.join(" ")}
        fill="none"
        stroke={color}
        strokeWidth="3"
        strokeLinejoin="round"
        strokeLinecap="round"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}
