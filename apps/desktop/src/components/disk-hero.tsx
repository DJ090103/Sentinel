import { motion } from "framer-motion";
import { ArrowRight } from "lucide-react";
import { Link } from "react-router";

import { CountUp } from "@/components/count-up";
import { Button } from "@/components/ui/button";
import type { DiskInfo } from "@/lib/system-info";
import { formatBytes } from "@/lib/system-info";

const RADIUS = 88;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

/**
 * The dashboard's one big moment: how much room is left, as a ring that fills
 * on arrival and a number that counts up to it. Everything else on the page is
 * deliberately quiet so this is what gets read first.
 */
export function DiskHero({ disk }: { disk: DiskInfo }) {
  const used = disk.totalBytes - disk.availableBytes;
  const usedFraction = disk.totalBytes === 0 ? 0 : used / disk.totalBytes;
  const tight = usedFraction >= 0.9;
  const freeGib = disk.availableBytes / 1024 ** 3;

  return (
    <section
      aria-label="Disk space"
      className="bg-card relative flex flex-col items-center gap-8 overflow-hidden rounded-[2rem] border-2 p-8 md:flex-row md:p-10"
    >
      <div
        aria-hidden="true"
        className="bg-primary/15 pointer-events-none absolute -top-24 -right-24 size-72 rounded-full blur-3xl"
      />

      <div className="relative size-52 shrink-0">
        <svg viewBox="0 0 200 200" className="size-full -rotate-90" aria-hidden="true">
          <circle
            cx="100"
            cy="100"
            r={RADIUS}
            fill="none"
            strokeWidth="16"
            className="stroke-muted"
          />
          <motion.circle
            cx="100"
            cy="100"
            r={RADIUS}
            fill="none"
            strokeWidth="16"
            strokeLinecap="round"
            className={tight ? "stroke-destructive" : "stroke-primary"}
            strokeDasharray={CIRCUMFERENCE}
            initial={{ strokeDashoffset: CIRCUMFERENCE }}
            animate={{ strokeDashoffset: CIRCUMFERENCE * (1 - usedFraction) }}
            transition={{ duration: 1.2, ease: [0.16, 1, 0.3, 1] }}
          />
        </svg>
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <span className="font-display text-5xl font-extrabold tracking-tight">
            <CountUp value={Math.round(usedFraction * 100)} />%
          </span>
          <span className="text-muted-foreground text-sm font-semibold">full</span>
        </div>
      </div>

      <div className="relative space-y-4 text-center md:text-left">
        <h1 className="font-display text-4xl leading-[1.05] font-extrabold tracking-tight md:text-5xl">
          <CountUp value={freeGib} format={(n) => n.toFixed(1)} /> GiB free
          <br />
          <span className="text-muted-foreground">of {formatBytes(disk.totalBytes)}</span>
        </h1>
        <p className="text-muted-foreground max-w-md text-base">
          {tight
            ? "This drive is nearly full. A scan shows what you can take back without touching your work."
            : "Run a scan to see which caches and leftovers are taking space you could have back."}
        </p>
        <Button asChild size="lg">
          <Link to="/system-scan">
            Scan this machine
            <ArrowRight aria-hidden="true" />
          </Link>
        </Button>
      </div>
    </section>
  );
}
