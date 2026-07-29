# What Sentinel touches, and what it never touches

Sentinel deletes files. This page is the complete, specific answer to
"what could it do to my machine?" — read it before you trust it, not
after.

If anything here turns out to be wrong, that is a security bug. See
[SECURITY.md](../SECURITY.md) for how to report it.

---

## The three rules

**1. Nothing is deleted that Sentinel didn't find itself.**

Sentinel can only delete a path that came out of its own scan, moments
earlier. There is no text box, no config option, and no setting anywhere
that lets a path be typed in and cleaned. The list you review is the list
that gets removed — and it is re-checked against a fresh scan immediately
before anything is deleted, so a directory that has since disappeared is
skipped rather than guessed at.

**2. Nothing is deleted without you ticking it and confirming.**

Three deliberate steps: open the page, tick the boxes, confirm. Never
one click. Nothing is deleted on a schedule or in the background — the
background job reads your CPU, memory and disk, and has no ability to
remove anything.

**3. Sentinel only knows a fixed list of directory names.**

It does not walk your caches deleting things that look disposable. Each
plugin has a hard-coded list of directory names, compiled into the app.
If your tools invent a new cache directory tomorrow, Sentinel will miss
it until someone adds it to that list — that is the intended trade.
Missing reclaimable space is recoverable; deleting your work is not.

---

## Risk levels

Every item Sentinel finds is labelled:

| Label | Meaning | Pre-ticked? |
| --- | --- | --- |
| **Safe** | Regenerated automatically. The worst case is a slower next build. | Yes |
| **Review** | Recoverable, but you'll notice — a longer rebuild, a re-download, a lost record. | No |
| **Destructive** | Could destroy something with no other copy. Requires typed confirmation. | No |

Only **Safe** items are ever ticked for you. A pre-ticked box is not a
decision you made.

No plugin shipping today produces a **Destructive** item — everything
currently offered is Safe or Review. The level and its typed-confirmation
flow exist and are wired up, ready for the higher-risk plugins on the
roadmap (Docker volumes being the obvious one).

---

## Per-tool detail

### npm
- **Offered:** the package cache (`_cacache`) and the npx cache.
- **Also offered, not pre-ticked:** `_logs` — records of past runs, which
  can't be recreated.
- Sentinel asks npm where its cache is (`npm config get cache`) rather
  than guessing.

### pip
- **Offered:** the download caches (`http-v2`, `http`, `selfcheck`).
- **Also offered, not pre-ticked:** `wheels` — these are wheels pip
  *built* on your machine. Removing them means recompiling C extensions
  (minutes for numpy or cryptography), not just re-downloading.

### Bun
- **Offered:** the install cache.
- A `BUN_INSTALL_CACHE_DIR` setting is **deliberately ignored**, and
  Sentinel tells you so. Honouring it would mean offering to delete a
  directory named entirely by configuration — a stray value pointing at
  your home directory would put your home directory on the review screen.

### Cargo
- **Offered:** the registry and git caches.
- **Never offered:** binaries you installed with `cargo install`.
  Per-project `target/` directories are out of scope entirely — Sentinel
  does not go looking through your projects.

### VS Code, Cursor, Kiro, Antigravity
These keep their caches in the *same* directory as your settings, so the
exclusion list is the safety property. **Never offered:**

- `User/` — `settings.json`, keybindings, snippets, `globalStorage`,
  `workspaceStorage`, and `History`
- `Backups/` — editor buffers you haven't saved yet
- Cursor's `snapshots/` — its record of previous states of your files.
  This is typically the *largest* directory there, and it is excluded
  precisely because it is your work rather than a cache
- `Partitions/`, `WebStorage/`, `Local Storage/`, `Session Storage/`,
  `IndexedDB/`, `Service Worker/` — extensions keep durable state here
- `Cookies`, `Local State`, `Preferences`, `machineid`

**Offered:** compiled script caches, GPU/graphics caches, network cache,
downloaded extension installers. **Offered but not pre-ticked:** `logs`
and `Crashpad` crash reports — the only evidence you'd have if you were
diagnosing a crash. On Kiro the log directory is often the biggest single
item Sentinel finds, so it is worth reading before you tick it.

### Browsers (Chrome, Chromium, Brave, Edge, Vivaldi, Firefox)
- **Offered:** cached pages, images and scripts.
- **Cannot reach, structurally:** history, cookies, saved passwords,
  bookmarks. On Linux these live in `~/.config/<browser>` while the cache
  lives in `~/.cache/<browser>` — a different directory tree. Sentinel
  builds every path from the cache root, so there is no expression in the
  code that reaches a profile. This isn't a promise not to; there is no
  route.
- **Firefox installed as a snap** — as it is by default on Ubuntu — is
  not covered, because a snap keeps its cache somewhere Sentinel doesn't
  look.

---

## What Sentinel never does

- **Never runs a shell command built from anything you or an AI typed.**
- **Never deletes on a timer.** The background job only reads.
- **Never sends your data anywhere.** There is no network call in the
  scanning or cleaning path at all. No telemetry, no accounts.
- **Never touches a project directory.** `node_modules/`, `target/`,
  `.git/` and your source tree are all out of scope.
- **Never asks for `sudo`,** and you should be suspicious if anything
  claiming to be Sentinel does. It only ever touches files your own user
  account owns.

---

## Verifying this yourself

Every claim above is pinned by a test you can run without installing
anything:

```bash
cargo test --workspace
```

The tests that matter here build a fake directory containing settings,
snippets, unsaved buffers and file history next to a cache, run a full
clean, and assert that everything except the cache survives.

To see exactly what Sentinel would offer on *your* machine, read-only,
without launching the app:

```bash
cargo test -p sentinel-plugin-vscode -- --ignored --nocapture
```
