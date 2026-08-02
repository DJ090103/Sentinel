# Sentinel architecture

This document is the public source of truth for Sentinel's current structure.
It describes code that exists in the repository today; planned work belongs in
issues or a separately reviewed proposal.

## Dependency direction

Sentinel is a Rust workspace wrapped in a Tauri desktop application:

```text
React UI
  -> Tauri commands and events
    -> sentinel-core
      -> sentinel-plugin-api
    -> statically linked plugin crates
      -> sentinel-plugin-api
```

`crates/core` never depends on Tauri or on a concrete plugin. It owns system
monitoring, settings, scheduling, plugin registration abstractions and merged
scan reports. `apps/desktop/src-tauri` is the composition root: it registers
the plugins shipped in the application and adapts core values to Tauri commands
and events.

Each crate under `plugins/` depends only on `crates/plugin-api`. Plugins are
compiled into the application; Sentinel does not load executable third-party
code at runtime.

## Scan and clean flow

1. The frontend invokes `scan_plugins` for a scan surface.
2. The desktop adapter moves the blocking filesystem work to Tokio's blocking
   pool.
3. Core scans each registered plugin and records completed, unavailable and
   failed outcomes independently.
4. The desktop adapter retains the report and returns it with a scan id.
5. The frontend sends back only the scan id, plugin id and selected positions.
   It never sends a path into Rust.
6. Rust resolves those positions against the retained report and invalidates
   the report before deletion.
7. `sentinel_plugin_api::clean` performs a fresh scan and rejects paths that no
   longer appear.
8. Only verified targets reach a plugin's removal implementation.

This proves provenance, not exclusive filesystem access. Files can still change
between verification and removal, so all metadata and removal code must remain
symlink-aware and fail closed.

## Plugin interface

The plugin trait is synchronous because its work is blocking filesystem I/O.
The caller—not each plugin—decides where that work runs. Keeping the trait
synchronous also keeps `plugin-api` independent of an async runtime.

Recommendations are explanations only. They contain no paths or commands and
cannot bypass the reviewed scan-and-clean flow.

See [PLUGIN_SDK.md](PLUGIN_SDK.md) for the plugin authoring contract and
[SAFETY.md](SAFETY.md) for the current inventories.

## Persistence

Settings are small, flat values stored as JSON in the operating system's
application-config directory. Writes use a temporary file followed by an
atomic rename. A malformed settings file is preserved before defaults are
loaded.

Live system information and scan reports are in memory. There is no database,
cleanup history or metrics time series. The `database` and `ai` modules are
placeholders and have no runtime callers.

The frontend currently stores its theme preference in webview `localStorage`,
while the Rust settings model also contains a theme field. That duplication is
a known cleanup item; code must not assume the two values are synchronized.

## Background work

The scheduler runs blocking jobs sequentially on Tokio's blocking pool. Its
single current job reads a system snapshot and records `last_scan_at`; it does
not scan plugins or clean anything. Job results are forwarded to the frontend
over the `sentinel://event` channel.

System CPU measurement is stateful. One shared `SystemMonitor` establishes a
baseline and is reused by dashboard polls and the background job.

## Frontend

The React application uses a hash router because Tauri's asset protocol has no
server-side fallback for deep links. Shared shadcn/ui primitives are vendored
under `apps/desktop/src/components/ui`, and Tailwind configuration is CSS-first
in `src/index.css`.

Rust is the source of truth for IPC payloads. TypeScript currently mirrors the
serialized types manually, so a wire-format change must update both sides and
the Rust serialization contract tests in the same change.

## Platform policy

Linux and macOS are the supported development targets. A plugin may support a
smaller set when the operating system does not provide a structurally safe
location. Unsupported behavior must return `ToolUnavailable` with a useful
reason rather than guessing.

Allocated size follows the host's reported filesystem blocks. Linux commonly
reports blocks for empty directories; APFS commonly reports zero, matching
macOS `du`. Sentinel stays conservative when the OS does not expose an
allocation.

Windows is intentionally deferred. Do not weaken the macOS/Linux interfaces in
anticipation of an untested Windows implementation.
