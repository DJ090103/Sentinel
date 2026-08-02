# Installing Sentinel

Sentinel is a desktop app. Published packages currently target Linux. macOS is
supported for development builds from source.

Before installing anything that deletes files, it's worth reading
[SAFETY.md](SAFETY.md) — it lists exactly what Sentinel will and won't
touch.

> Filenames below use `0.1.0`. Substitute whatever version you actually
> downloaded.

---

## Option 1 — `.deb` (Debian, Ubuntu, Pop!_OS, Mint)

Download the `.deb` from the [latest
release](https://github.com/DJ090103/Sentinel/releases/latest), then:

```bash
sudo apt install ./Sentinel_0.1.0_amd64.deb
```

`apt install ./file.deb` rather than `dpkg -i` so dependencies are
resolved for you. Sentinel then appears in your applications menu.

To remove it:

```bash
sudo apt remove sentinel
```

## Option 2 — AppImage (any distro)

Download the `.AppImage`, make it executable, and run it:

```bash
chmod +x Sentinel_0.1.0_amd64.AppImage
./Sentinel_0.1.0_amd64.AppImage
```

No installation and no root. If it doesn't start, you may need FUSE:

```bash
sudo apt install libfuse2        # Debian/Ubuntu
sudo dnf install fuse-libs       # Fedora
```

## Option 3 — `.rpm` (Fedora, RHEL, openSUSE)

```bash
sudo dnf install ./Sentinel-0.1.0-1.x86_64.rpm
```

---

## Building from source

Needed on Arch, NixOS, or anywhere the packages above don't fit — and if
you'd rather build a file-deleting tool yourself than trust a binary,
which is a reasonable position.

**Prerequisites:**

- [Rust](https://rustup.rs) 1.96.1 (installed automatically by rustup from
  `rust-toolchain.toml`)
- [Node.js](https://nodejs.org) 22.13+ and [pnpm](https://pnpm.io) 11.7.0
- Tauri's platform prerequisites:

On macOS, install Apple's command-line tools:

```bash
xcode-select --install
```

On Linux, install the WebKitGTK and packaging libraries:

```bash
# Debian / Ubuntu
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev \
  librsvg2-dev patchelf libgtk-3-dev build-essential curl wget file

# Fedora
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel patchelf gtk3-devel

# Arch
sudo pacman -S webkit2gtk-4.1 base-devel curl wget file openssl \
  appmenu-gtk-module libappindicator-gtk3 librsvg patchelf
```

See [Tauri's prerequisites
guide](https://tauri.app/start/prerequisites/) if your distro isn't
listed.

**Then:**

```bash
git clone https://github.com/DJ090103/Sentinel.git
cd Sentinel
corepack enable
corepack install --global pnpm@11.7.0
pnpm install --frozen-lockfile

# Run it in development
pnpm --filter sentinel-desktop tauri dev

# Or build an installable bundle
pnpm --filter sentinel-desktop tauri build
```

The root `package.json` pins pnpm. If `pnpm` still resolves to an old or deleted
global install, inspect `which -a pnpm`, remove that stale shim from your `PATH`,
then rerun the two Corepack commands above.

Bundles land in `target/release/bundle/` — `deb/`, `rpm/` and
`appimage/` subdirectories on Linux, with the platform-appropriate bundle
directory on macOS.

macOS development builds are supported for local testing. Published macOS
artifacts are not provided yet because signing and notarization have not been
configured.

**Verifying before you trust it:**

```bash
cargo test --workspace
```

This runs the tests that assert Sentinel never proposes your settings,
snippets, unsaved buffers, browser passwords or file history.

---

## Where Sentinel keeps its own files

One plain JSON file, which you can read, edit, or delete to reset the app:

- Linux: `${XDG_CONFIG_HOME:-~/.config}/sentinel/settings.json`
- macOS: `~/Library/Application Support/sentinel/settings.json`

There is no database, no cache of your data, and nothing sent anywhere.

---

## Uninstalling completely

Remove the installed package with your distro's package manager, then remove
the platform-specific settings directory above if you also want to reset local
configuration.

That's everything. Sentinel doesn't install services, timers, or
autostart entries.
