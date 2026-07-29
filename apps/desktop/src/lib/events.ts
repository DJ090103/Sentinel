/**
 * TypeScript mirror of `AppEvent` in `crates/core/src/notifications/mod.rs`.
 *
 * Hand-written for the same reason as `system-info.ts`: one event type doesn't
 * justify a codegen step yet, and the tradeoff is the same — the Rust side is
 * the source of truth, so if you change it, change this too. A Rust test pins
 * the serialized shape (`serializes_as_a_tagged_union_the_frontend_can_switch_on`)
 * so a drift shows up as a failing test rather than a silently dead listener.
 */

/** The single Tauri channel every app event arrives on. */
export const APP_EVENT_NAME = "sentinel://event";

/** How a scheduled job ended. Mirrors `JobOutcome`. */
export type JobOutcome =
  | { status: "succeeded" }
  | { status: "failed"; detail: string }
  | { status: "panicked"; detail: string };

/** One completed run of a scheduled job. Mirrors `JobRun`. */
export interface JobRun {
  jobId: string;
  /** RFC 3339. */
  startedAt: string;
  /** RFC 3339. */
  finishedAt: string;
  outcome: JobOutcome;
}

/**
 * Anything the background half reports.
 *
 * A discriminated union on `type`, so new variants can be added on the Rust
 * side without breaking a frontend that only handles the ones it knows.
 */
export type AppEvent = { type: "jobRan"; run: JobRun };

/**
 * Renders a timestamp for display, or `null` if it can't be parsed.
 *
 * Absolute rather than relative ("3 minutes ago") on purpose: a relative label
 * is only truthful if something re-renders it on a timer, and this value comes
 * from an event precisely so the dashboard doesn't need one. An absolute time
 * stays correct sitting on screen untouched.
 *
 * Today's scans show the time alone, since the date would be noise; anything
 * older carries its date, because "at 14:32" without one is ambiguous after the
 * app has been open overnight.
 */
export function formatScanTime(rfc3339: string, now: Date = new Date()): string | null {
  const at = new Date(rfc3339);
  if (Number.isNaN(at.getTime())) {
    return null;
  }

  const sameDay =
    at.getFullYear() === now.getFullYear() &&
    at.getMonth() === now.getMonth() &&
    at.getDate() === now.getDate();

  // `undefined` locale means the user's own, which is what a local-first
  // desktop app should follow rather than pinning en-US.
  return new Intl.DateTimeFormat(undefined, {
    timeStyle: "short",
    ...(sameDay ? {} : { dateStyle: "medium" }),
  }).format(at);
}

/** A human-readable reason for a run that didn't succeed, or `null`. */
export function failureDetail(outcome: JobOutcome): string | null {
  switch (outcome.status) {
    case "succeeded":
      return null;
    case "failed":
      return outcome.detail;
    case "panicked":
      // Worded so the user knows this one is a defect rather than a condition
      // of their machine, without showing them a Rust panic verbatim.
      return `${outcome.detail} (this is a bug in Sentinel)`;
  }
}
