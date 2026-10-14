# Changelog

All notable changes to this project are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.0] - 2026-10-14

### Added
- Desktop companion: an optional floating pixel monitor (its own transparent window) that reacts to CPU, memory, open apps, idle time and the app in front. Configure it in Settings (show/hide, name, size, shape, colours, always on top, start at login, thresholds)
- Dashboard: live CPU and memory charts, a plain-English health verdict, and a usage profile built from which kind of app you use
- Redesigned interface: indigo-and-gold theme, chunky tactile buttons, animated navigation and page transitions, a disk ring on the dashboard, and a celebratory result when a clean finishes (all respect reduced motion)
- Insights page: total reclaimed, cleaning streak, 30-day disk usage trend and cleanup history, backed by a local SQLite database the background job writes to
- System Scan: one button now also analyzes your home folder (biggest folders, largest files, exact duplicates) with live progress and cancel; report only, nothing is deleted
- System Scan lists old temp files (`/tmp`, `/var/tmp`, `%TEMP%`) and Windows caches for review; nothing is ticked for you
- Workspace scaffold: `apps/desktop` (Tauri + React), `crates/core`, `crates/plugin-api`
- CI (fmt, clippy, test, frontend lint/build)
- Project documentation structure (`docs/`)
- Tailwind CSS v4 and shadcn/ui in the desktop app, with a light/dark design
  token theme and the shared `Button` component
- Dark mode: a light/dark/system theme switcher. The choice is remembered
  between launches, defaults to following the operating system, and is applied
  before the window paints so there's no flash of the wrong theme on startup
- App shell: a persistent sidebar with navigation between Dashboard, Developer
  Health, Insights and Settings. The pages are placeholders for now; the theme
  switcher has moved to Settings
- A "Page not found" screen, keeping the sidebar available, for addresses that
  don't match any page
- Dashboard now shows live system health — CPU load and core count, memory,
  disk usage for the primary filesystem, operating system and hostname —
  refreshed every two seconds. Anything the OS can't report reads as
  "Unknown" rather than being filled in with a placeholder
- Sentinel now checks the machine in the background on a schedule (every five
  minutes by default) and remembers when it last did. Preferences and that
  timestamp live in a plain JSON file at `~/.config/sentinel/settings.json`
  (equivalent locations on macOS and Windows) that you can read or edit
  yourself. If that file is ever unreadable, Sentinel keeps a copy of it
  alongside as `settings.<timestamp>.json.corrupt` and starts from defaults
  rather than overwriting what you had. Nothing is scanned or deleted — this
  only records the same readings the dashboard already shows
- The Dashboard now says when that background check last ran, updating by
  itself as each one finishes. If a check fails it says so and keeps the
  earlier time, rather than implying a check happened when it didn't
- Developer Health is a real page: it scans your developer tools' caches,
  shows what each is costing, and can reclaim the ones you choose. Nothing is
  removed without three deliberate steps — scan, tick, confirm — and the
  confirmation lists the exact directories first. Only regenerable caches are
  ticked for you; anything you'd notice losing is left for you to choose
  deliberately. Sentinel only ever offers directories it knows about, so
  anything else living alongside them is left alone
- Developer Health covers **npm, pip, Bun, Cargo, VS Code, Cursor, Kiro,
  Antigravity and browser caches**. What each one will and won't touch:
  - **npm** — the package and npx caches. Debug logs are offered but never
    pre-ticked: unlike a cache, a record of past runs can't be recreated
  - **pip** — the download caches. Locally *built* wheels are offered but
    never pre-ticked, because recreating those means recompiling rather than
    re-downloading
  - **Bun** — the install cache. A `BUN_INSTALL_CACHE_DIR` setting is
    deliberately ignored, and Sentinel tells you so, rather than offering to
    delete a directory named entirely by configuration
  - **Cargo** — the registry and git caches. Binaries you installed with
    `cargo install` are never offered; per-project `target/` directories are
    out of scope
  - **VS Code, Cursor, Kiro and Antigravity** — caches only. Your settings,
    snippets, workspace state, unsaved editor buffers and Cursor's
    file-history snapshots are never offered. Editor logs and crash reports
    are offered but never pre-ticked, which matters most for Kiro, where the
    log directory is typically the largest thing Sentinel finds
  - **Browser caches** — cached pages, images and scripts for Chrome,
    Chromium, Brave, Edge and Vivaldi. History, cookies, saved passwords and
    bookmarks live in a separate directory that Sentinel has no way to reach
    at all. Firefox is recognised too, but not yet when it's installed as a
    snap — as it is by default on Ubuntu — because a snap keeps its cache
    somewhere Sentinel doesn't look. Not available on Windows at all, where
    the cache sits inside the profile folder and that separation doesn't
    exist
- Sentinel has its own icon. It appears in your applications menu once
  installed, on the window, and on the browser tab in development — and the
  mark now sits in the app's own sidebar. The installed entry is filed under
  Development and carries a real description, rather than appearing
  uncategorised and unlabelled

- Linux release builds. Tagging `v*` builds `.deb`, `.rpm` and AppImage
  bundles and attaches them to a draft GitHub release, after running the
  full test suite — a build that fails a safety test is never published.
  Built on Ubuntu 22.04 so the bundles also run on older distributions
- Documentation aimed at people installing Sentinel rather than building
  it: `docs/INSTALL.md` for every install route including from source, and
  `docs/SAFETY.md` listing exactly what each plugin touches and what it can
  never reach. `SECURITY.md` describes how to report a way of making
  Sentinel delete something it shouldn't

### Changed
- Window title is now "Sentinel" rather than the project scaffold's default
- The README now describes what Sentinel actually does today, and says
  plainly what it doesn't do yet — the AI layer is still a deliberate stub,
  Insights is a placeholder, and several planned plugins don't exist

### Removed
- The Tauri scaffold's demo screen (logos, greet form) and its `App.css`, whose
  hardcoded light-only colors conflicted with the new theme
- The scaffold's leftover Vite, Tauri and React logos, which nothing referenced
  any more
