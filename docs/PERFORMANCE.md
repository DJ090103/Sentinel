# Performance verification

Sentinel's meaningful cost is filesystem work, not arithmetic. Performance
changes must therefore be measured through complete scan flows rather than a
microbenchmark of path joining or byte formatting.

## Baseline dimensions

Record these separately for macOS and Linux before changing scan orchestration:

- application cold-start time;
- idle CPU and resident memory over 60 seconds;
- dashboard-visible CPU over 60 seconds;
- wall time per plugin scan;
- number of filesystem entries visited per plugin;
- time until the first plugin result is available;
- cancellation latency;
- number of plugin scans triggered by page open, recommendation loading and
  cleanup;
- frontend production bundle size.

Use the same fixture or real cache snapshot when comparing two revisions. A
faster result over fewer files is not a performance improvement.

## Current verified baseline

On an Apple Silicon development machine with Node 22.22.2 and pnpm 11.7.0:

- frontend lint passes;
- the production frontend builds in approximately 1.3 seconds with warm
  dependencies;
- JavaScript output is approximately 347 KiB raw and 111 KiB gzip;
- CSS output is approximately 23 KiB raw and 5 KiB gzip.

These figures are diagnostic context, not CI limits. Hardware-independent CI
should assert behavior such as one active scan per surface and one scan per
user action. Relative benchmark thresholds can be added once stable fixtures
exist.

## Optimization rules

- Measure before introducing concurrency.
- Prefer eliminating a filesystem walk over making the walk more complicated.
- Never cache a deletion authorization; cleanup still requires a fresh scan.
- Bound concurrency because plugins contend for one disk.
- Cancellation must be cooperative inside directory traversal; dropping a
  frontend promise does not stop a blocking Rust task.
- Pause live polling while the window is hidden.
- Do not trade a deletion invariant for a performance gain.
