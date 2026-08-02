# Contributing to Sentinel

Contributions are welcome. Sentinel deletes files for a living, so the
bar for anything near that code path is higher than usual — this page is
about where that bar sits.

## The most useful thing you can do

**Add a plugin for a tool Sentinel doesn't cover yet.** Docker, Git,
Playwright, Android Studio, Ollama, Hugging Face, JetBrains IDEs, Gradle,
Maven, Go, Deno — all fair game and all self-contained. A plugin lives in
its own crate under `plugins/`, depends only on `crates/plugin-api`, and
implements one trait.

Open an issue before you start so we don't both write the same one.

## Setup

Sentinel pins Rust 1.96.1 and pnpm 11.7.0. pnpm 11 requires Node.js
22.13 or newer. Rustup reads `rust-toolchain.toml` automatically; the root
`package.json` records the Node and pnpm requirements.

```bash
git clone https://github.com/DJ090103/Sentinel.git
cd Sentinel
corepack enable
corepack install --global pnpm@11.7.0
pnpm install --frozen-lockfile
pnpm --filter sentinel-desktop tauri dev
```

Tauri needs some system libraries first — see
[docs/INSTALL.md](docs/INSTALL.md#building-from-source).

`crates/core` and `crates/plugin-api` build anywhere with just Rust; only
the desktop app needs the GTK packages.

## Before you open a PR

All of these must pass — CI runs them, so it's faster to run them
yourself:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --filter sentinel-desktop lint
pnpm --filter sentinel-desktop build
```

## House rules

These are non-negotiable, and a PR that breaks one won't be merged
regardless of how good the rest of it is:

1. **No arbitrary command execution.** Nothing shells out to a command
   built from a string. Where a plugin does invoke a tool (npm, for
   instance), the argv is fixed at compile time.
2. **A plugin proposes a closed list of names it knows.** It does not
   walk a directory deleting what looks disposable. If an upstream tool
   adds a new cache directory, Sentinel misses it until someone adds the
   name — that's the intended trade.
3. **A plugin never proposes the root it discovered** — only known
   children of it that already exist.
4. **`crates/core` never depends on `tauri`.** Anything needing a Tauri
   type belongs in `apps/desktop/src-tauri`, calling into `core`.
5. **No `unwrap()` or `expect()`** outside genuinely infallible cases,
   and an `expect()` must say in its message *why* it can't fail.
6. **Public items get doc comments explaining why they exist,** not
   restating the signature.

## Writing a plugin

The bar, in one paragraph: your plugin's exclusion list is a safety
claim, so it needs a test that proves it. Build a fixture directory
containing the things a user would be upset to lose — settings, history,
credentials, unsaved work — next to the cache you're targeting, run a
full clean, and assert everything except the cache survives.
`plugins/vscode/src/lib.rs` is the worked example; its
`nothing_the_user_would_miss_is_ever_proposed` test is the shape to copy.

Label risk honestly. `Safe` means regenerated automatically with no
user-visible loss beyond a slower next build. Anything a user would
*notice* losing — logs, crash reports, locally built artifacts — is
`Review` and never pre-ticked. When you're unsure between two levels,
pick the more cautious one.

## Commits

[Conventional Commits](https://www.conventionalcommits.org/): `feat:`,
`fix:`, `docs:`, `refactor:`, `chore:`, `test:`.

## UI changes

Dark mode has to work, loading and error states have to be handled (not
just the happy path), and new components should match the patterns
already in `apps/desktop/src` rather than introducing a new one.

## Security

Don't open a public issue for anything that gets Sentinel to delete
something it shouldn't — see [SECURITY.md](SECURITY.md) for private
reporting.

## License

Contributions are MIT, same as the project.
