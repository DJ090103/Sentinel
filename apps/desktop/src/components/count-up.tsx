import { animate, useMotionValue, useReducedMotion } from "framer-motion";
import { useEffect, useState } from "react";

/**
 * A number that counts to its value rather than snapping to it.
 *
 * The count is what makes a reclaimed total feel earned, so it is used for
 * results and not for values that change every poll. With reduced motion the
 * final value is shown at once.
 */
export function CountUp({
  value,
  format = (n) => Math.round(n).toLocaleString(),
  duration = 1.1,
}: {
  value: number;
  format?: (n: number) => string;
  duration?: number;
}) {
  const reduce = useReducedMotion();
  const motion = useMotionValue(0);
  const [shown, setShown] = useState(0);

  useEffect(() => {
    if (reduce) {
      return;
    }
    const unsubscribe = motion.on("change", setShown);
    const controls = animate(motion, value, { duration, ease: [0.16, 1, 0.3, 1] });
    return () => {
      controls.stop();
      unsubscribe();
    };
  }, [value, duration, reduce, motion]);

  return <span className="tabular-nums">{format(reduce ? value : shown)}</span>;
}
