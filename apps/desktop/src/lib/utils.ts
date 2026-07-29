import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * Merges Tailwind class names, resolving conflicts in favour of the last one.
 *
 * Exists because every shadcn/ui component accepts a `className` prop that must
 * be able to *override* the component's own defaults. Plain string
 * concatenation can't do that — `"p-2" + " p-4"` leaves both classes present
 * and lets CSS source order decide the winner, which is not what the caller
 * meant. `twMerge` understands Tailwind's conflict groups and drops the loser.
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
