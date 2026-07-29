# Security Policy

## Supported versions

Sentinel is pre-1.0. Only the latest release gets fixes.

| Version | Supported |
| --- | --- |
| 0.1.x | ✅ |
| < 0.1 | ❌ |

## What counts as a security bug here

Sentinel deletes files, so the bug class that matters most is anything
that gets it to delete something it shouldn't. Specifically:

- **A path Sentinel didn't find itself reaching the delete path.**
  Everything removed must have come out of Sentinel's own scan. If you
  find a route around that — through the UI, through a config file
  Sentinel reads, through an environment variable, through a symlink —
  that's the highest-severity report this project can receive.
- **A plugin proposing something outside its documented list.** Every
  exclusion in [docs/SAFETY.md](docs/SAFETY.md) is a claim. A way to make
  a plugin offer `User/`, `snapshots/`, a browser profile, or anything
  else on those never-offered lists is a security bug, not a feature
  request.
- **A tool's own config redirecting Sentinel somewhere dangerous.**
  Plugins ask tools where their caches are, and those answers come from
  files Sentinel didn't write. If a crafted config makes Sentinel propose
  a directory that matters, report it.
- **Any subprocess invocation built from unvalidated input.**
- **Anything that makes Sentinel send data off the machine.** There
  should be no network call in the scan or clean path at all.

Also in scope: privilege escalation, and anything that would let a
downloaded release differ from what this repository builds.

## Reporting

**Please don't open a public issue for these.** Use GitHub's private
reporting instead:

> Repository → **Security** tab → **Report a vulnerability**

That opens a private advisory only maintainers can see.

Useful in a report, roughly in order of usefulness:

1. What you got Sentinel to delete, or nearly delete
2. The steps — a failing test case is ideal, since every safety property
   in this project is pinned by one
3. Your OS, distro and Sentinel version
4. Whether it needs an unusual config, or works on a default install

You'll get an acknowledgement within a few days. This is a small project
with no dedicated security team, so please be patient — but a report that
a delete path can be subverted will be treated as the most urgent thing
in the queue.

## Disclosure

Report privately, give a reasonable window for a fix, and you'll be
credited in the release notes and the advisory unless you'd rather not
be. There's no bounty programme.

## What's out of scope

- Sentinel deleting exactly what you ticked and confirmed. That's the
  feature.
- Vulnerabilities in Tauri, WebKitGTK or other dependencies — report
  those upstream, though a heads-up here is welcome so the dependency can
  be bumped.
- Anything requiring an attacker who already has code execution as your
  user. At that point they don't need Sentinel.

## How the guarantees are enforced

For context on what you'd be trying to break, the delete path is designed
so the checks can't be skipped rather than merely documented:

- A target can only be produced by a plugin's own `scan()`, and carries
  which plugin made it
- Targets are serialized *outward* to the UI only, never accepted back —
  so a path cannot enter the app from the webview at all. The frontend
  sends positions in a scan the backend already holds
- The type representing "verified, safe to delete" cannot be constructed
  outside the API crate, so no plugin can be handed an unchecked path
- Every target is re-verified against a fresh scan immediately before
  removal

If you find a gap in any of that, it's exactly what this policy is for.
