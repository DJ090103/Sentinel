# Plugin authoring contract

A Sentinel plugin recognizes removable data for one tool. It lives in its own
crate under `plugins/`, depends only on `sentinel-plugin-api`, and implements
the `Plugin` trait.

## Required properties

- Use a stable plugin id. Once released it is persistent identity.
- Resolve the tool root from an operating-system convention or from the tool
  itself, then validate it with `validate_tool_directory`.
- Propose a closed inventory of compile-time-known children. Never delete an
  unknown sibling because it looks cache-like.
- Never propose the discovered root itself unless the target path is derived
  entirely from constant components, as with Bun's `install/cache`.
- Inspect candidates with symlink-aware metadata and never follow a symlink.
- Return `ToolUnavailable` when the tool or supported location does not exist.
- Treat one target's removal failure as partial failure, not a reason to hide
  successful removals.
- Use fixed subprocess executables and argument vectors. Never invoke a shell
  or interpolate output into a command.

## Risk

- `Safe`: automatically regenerated; only the next operation becomes slower.
- `Review`: removal is recoverable or intentional, but the user will notice.
- `Destructive`: removal may lose the only copy or break a running system.

Only `Safe` may be preselected. No current shipped plugin emits a destructive
target.

## Tests

Every plugin must prove its exclusions. Build a temporary fixture containing
the intended cache beside settings, credentials, history, unsaved work and an
unknown directory. Scan and clean through the public plugin interface, then
assert that only the intended target disappeared.

Also cover:

- missing and empty roots;
- foreign and fabricated targets;
- stable metadata;
- risk classification;
- platform-specific path layouts;
- recommendations containing no executable path;
- partial cleanup failure where applicable.

`plugins/vscode/src/lib.rs` is the strongest worked example because its fixture
places caches beside editor settings, backups and file history.

Real-machine scans must remain ignored, read-only tests that require an
explicit `--ignored --nocapture` invocation.
