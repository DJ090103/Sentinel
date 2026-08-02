# Sentinel

**A developer workstation companion that explains before it deletes.**

Sentinel shows you what your developer tools are costing in disk space —
npm, pip, Bun, Cargo, your editors, your browsers — and lets you reclaim
it. It never cleans anything on its own. You tick what goes, and you
confirm it.

![License](https://img.shields.io/badge/license-MIT-blue)
![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20dev-informational)
![Status](https://img.shields.io/badge/status-early%20release-orange)

---

## Why another cleanup tool

Most of them are a black box: one big button, and you hope. That's a bad
trade on a machine where a wrong delete costs you a day — your editor
settings, your unsaved buffers, your browser logins.

Sentinel's position is the opposite. It only knows a fixed list of
directories, it tells you what each one is and what you lose by removing
it, and it can't touch anything it didn't find itself:

> **Cursor — compiled script cache (CachedData), 129.9 MiB**
> Pre-compiled JavaScript the editor generates to start faster. Rebuilt
> on the next few launches, which are slightly slower.

You decide. Sentinel explains.

## What it does today

- **Dashboard** — live CPU, memory, disk, OS and hostname, refreshed
  every two seconds
- **Developer Health** — scans npm, pip, Bun, Cargo, VS Code, Cursor,
  Kiro, Antigravity and browser caches; shows what each is worth
  reclaiming; cleans only what you tick and confirm
- **Dark mode**, following your system by default
- **Background monitoring** — checks the machine on a schedule and
  remembers when it last did. It only ever *reads*; it never cleans

## What it doesn't do yet

Being straight about this, since the app is young:

- **No AI explanations yet.** That's the intent, but the AI layer is a
  deliberate stub — nothing calls a model, and nothing will until that
  boundary has been reviewed. What you get today is hand-written
  explanations, not generated ones.
- **No history or trends.** The Insights page is a placeholder.
- **No Docker, Git, Playwright, Android Studio, Ollama or Hugging Face**
  plugins yet.
- **Linux releases only.** macOS development builds are covered by the test
  matrix, but signed and notarized macOS artifacts are not released yet.
  Windows support is deferred.

## Install on Linux

```bash
sudo apt install ./Sentinel_0.1.0_amd64.deb
```

Full instructions for `.deb`, AppImage, `.rpm` and building from source:
**[docs/INSTALL.md](docs/INSTALL.md)**

macOS developers can run Sentinel from source; published macOS packages will
follow after signing and notarization are configured.

For maintainers, **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** explains
the runtime and dependency seams, and **[docs/PLUGIN_SDK.md](docs/PLUGIN_SDK.md)**
defines the safety contract for adding a tool.

## Is it safe?

It deletes files, so you should want a real answer rather than a
reassurance. **[docs/SAFETY.md](docs/SAFETY.md)** lists exactly what each
plugin touches and what it can never reach. The short version:

- Nothing is deleted that Sentinel didn't find in its own scan
- Nothing is deleted without you ticking it and confirming
- Nothing is deleted on a timer, ever
- No shell commands built from user or AI input
- No network calls in the scan or clean path — no telemetry, no accounts
- Your settings, snippets, unsaved buffers, file history, browser
  passwords, cookies and bookmarks are never offered

Those aren't promises in a document — they're properties with tests. Run
`cargo test --workspace` and you'll see the ones that build a fake
settings directory and assert it survives a full clean.

## Built with

Rust · [Tauri v2](https://tauri.app) · React · TypeScript · Tailwind ·
shadcn/ui

Everything runs locally. There is no server.

## Contributing

Contributions are welcome, with one caveat: the plugin API is still
settling, so check [CONTRIBUTING.md](CONTRIBUTING.md) before starting
anything large. Adding a plugin for a tool Sentinel doesn't cover yet is
the most useful thing you can do, and the easiest to review.

## Security

Found a way to make Sentinel delete something it shouldn't? That's the
bug class this project cares about most — see [SECURITY.md](SECURITY.md).

## License

MIT
