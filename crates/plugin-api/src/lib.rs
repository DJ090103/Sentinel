//! `sentinel-plugin-api` defines the contract every Sentinel plugin
//! implements — npm, Bun, Cargo, Docker, browser caches, Cursor,
//! VS Code, Android Studio, and anything added later.
//!
//! This crate is intentionally tiny. Plugin crates (under `/plugins` at the
//! repo root) depend on this crate alone, not on `sentinel-core`. That means a
//! plugin author implementing, say, the Docker plugin never pulls in the
//! scheduler or database code just to satisfy the trait — and `sentinel-core`
//! can depend on this crate to define `PluginRegistry` without a circular
//! dependency back to any individual plugin.
//!
//! # Why the trait is synchronous
//!
//! An earlier sketch of this API used `async fn` methods. It can't work as
//! written: `sentinel-core`'s registry stores `Box<dyn Plugin>`, and a trait
//! with native `async fn` is not dyn-compatible, so it cannot be boxed. The
//! ways out are to depend on `async-trait` (a proc-macro dependency in the
//! crate whose whole purpose is to stay minimal) or to hand-write methods
//! returning `Pin<Box<dyn Future>>` in every plugin.
//!
//! Neither is worth it, because plugin work is filesystem-bound — walking cache
//! directories, stat-ing files, deleting them. That is blocking I/O whichever
//! syntax wraps it. The caller decides how to get it off the main thread; see
//! `sentinel_core::scanner`.
//!
//! # The safety contract
//!
//! Deleting files is the only irreversible thing Sentinel does, so the contract
//! around it is enforced by the type system rather than by asking twelve plugin
//! authors to each remember the same three rules.
//!
//! The mechanism is [`VerifiedTarget`]: a wrapper whose field is private to
//! this crate, so nothing outside it can construct one. The free function
//! [`clean`] performs the checks and hands the results to
//! [`Plugin::remove_verified`] — the only method a plugin implements that
//! deletes, and one that cannot be called with anything unverified because
//! there is no other way to obtain a `VerifiedTarget`.
//!
//! `clean` is a free function rather than a method on [`Plugin`] on purpose. A
//! provided trait method can be overridden, and an override would skip every
//! check — which would leave the delete path's safety resting on a "do not
//! override" comment, exactly the weakness this design replaced. There is no
//! `plugin.clean(..)` to override.
//!
//! Around that:
//!
//! - A [`CleanTarget`] can only be produced by [`Plugin::scan`], and records
//!   which plugin produced it.
//! - [`CleanTarget`] is deliberately **not** `Deserialize`. Targets are sent to
//!   the UI for review, never accepted back from it — otherwise anything that
//!   could reach a Tauri command could mint a target with a path of its
//!   choosing. See that type for the full reasoning.
//! - There is deliberately no API that takes a path as a string. Nothing in
//!   Sentinel ever deletes a path that didn't come out of a `scan()`.
//!
//! What this does *not* claim: the filesystem can still change between the
//! verifying scan and the removal. This guarantees a path's provenance, not
//! exclusive access to it.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Measuring how much a directory would actually free. Shared so every plugin
/// reports space the same way — see the module docs for why that matters.
pub mod sizing;

pub use sizing::{directory_size, file_size, is_empty_directory};

/// Static identity of a plugin, shown in the UI and used for lookup.
///
/// The fields are `&'static str` because every plugin is compiled into the
/// binary — there is no dynamic loading, so these are always literals. That
/// makes `PluginMetadata` `Copy` and free to hand out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMetadata {
    /// Stable machine identifier, e.g. `"npm"`.
    ///
    /// Used as the registry key and stored in cleanup history, so changing it
    /// for an existing plugin orphans that plugin's records. Treat it as
    /// permanent once shipped.
    pub id: &'static str,
    /// Human-readable name, e.g. `"npm cache"`.
    pub name: &'static str,
    /// One line on what this plugin looks at and why it's worth cleaning.
    pub description: &'static str,
}

/// How much care a target needs before it's removed.
///
/// This drives how the UI presents a target — what's pre-selected, what
/// demands a deliberate opt-in. It is the plugin author's judgement about
/// consequences, not about size. A 40 GiB npm cache is [`Safe`](Self::Safe); a
/// 3 MiB Docker volume that is the only copy of someone's database is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TargetRisk {
    /// Regenerable with no user-visible loss beyond a slower next build.
    /// A package manager's download cache is the archetype.
    Safe,
    /// Recoverable, but the user will notice — a longer rebuild, a re-download
    /// over a metered connection, a lost warm start.
    Review,
    /// Removal may destroy something with no other copy, or break a running
    /// system.
    ///
    /// Reserved for cases where being wrong costs the user real work, not just
    /// time: a Docker volume, a browser profile directory, anything belonging
    /// to a process still running. See `docs/PLUGIN_SDK.md` for the risk
    /// contract plugins must follow.
    ///
    /// A plugin that is unsure whether something is `Review` or `Destructive`
    /// should choose `Destructive`. The cost of over-warning is a moment's
    /// friction; the cost of under-warning is unrecoverable.
    Destructive,
}

impl TargetRisk {
    /// Whether the UI must demand a typed confirmation rather than a checkbox.
    ///
    /// A method rather than each screen matching on the variant itself: adding
    /// a risk level later should not mean auditing every call site to find the
    /// ones that silently treated the new variant as harmless.
    pub fn requires_typed_confirmation(self) -> bool {
        matches!(self, Self::Destructive)
    }

    /// Whether a target may be pre-selected for the user.
    ///
    /// Only [`Safe`](Self::Safe) is. Everything else is an opt-in the user has
    /// to make deliberately — a pre-ticked box is not a decision.
    pub fn may_be_preselected(self) -> bool {
        matches!(self, Self::Safe)
    }
}

/// One thing a plugin found that could be removed.
///
/// Only [`Plugin::scan`] produces these. [`CleanTarget::new`] requires the
/// producing plugin's id, so a target always carries provenance — see the
/// crate-level safety contract.
///
/// # Why this is `Serialize` but deliberately not `Deserialize`
///
/// Deriving `Deserialize` would rebuild a target field by field, bypassing
/// [`CleanTarget::new`] entirely — anything able to send JSON to a Tauri
/// command could then mint a target with an arbitrary `path` and a matching
/// `plugin_id`, and the provenance guarantee above would be fiction.
///
/// Targets therefore travel *outward* only. A UI that needs the user to pick
/// from a scan must send back identifiers the Rust side can resolve against the
/// scan it already holds, never the targets themselves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanTarget {
    /// Which plugin found this. `clean()` refuses targets that aren't its own.
    plugin_id: String,
    /// Absolute path that would be removed.
    path: PathBuf,
    /// What the user sees, e.g. `"npm cache (_cacache)"`.
    pub label: String,
    /// Size on disk at scan time. May be stale by the time `clean` runs, which
    /// is one reason `clean` re-scans rather than trusting this.
    pub size_bytes: u64,
    /// How much care this target needs.
    pub risk: TargetRisk,
}

impl CleanTarget {
    /// Creates a target. Called by a plugin's `scan`, and nowhere else.
    ///
    /// Takes `plugin_id` so provenance can't be omitted: a target that doesn't
    /// know where it came from can't be checked against the plugin asked to
    /// delete it.
    pub fn new(
        plugin_id: impl Into<String>,
        path: impl Into<PathBuf>,
        label: impl Into<String>,
        size_bytes: u64,
        risk: TargetRisk,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            path: path.into(),
            label: label.into(),
            size_bytes,
            risk,
        }
    }

    /// The plugin that produced this target.
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// The path that would be removed.
    ///
    /// Read-only by design: a caller can inspect the path but cannot swap it
    /// for another one after the user has reviewed the list.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// What a plugin found.
///
/// Not `Deserialize`, because it carries [`CleanTarget`]s — see that type for
/// why they must never be reconstructed from untrusted input.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    /// Everything removable that was found. Empty is a valid, successful
    /// result — it means there's nothing to clean, not that scanning failed.
    pub targets: Vec<CleanTarget>,
}

impl ScanResult {
    /// Total size of every target found.
    pub fn total_bytes(&self) -> u64 {
        // `saturating_add` rather than `+`: these are summed from filesystem
        // sizes, and a bogus report shouldn't be able to panic a release build
        // by overflowing.
        self.targets
            .iter()
            .fold(0u64, |sum, target| sum.saturating_add(target.size_bytes))
    }
}

/// How much space a plugin believes it could reclaim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceEstimate {
    /// Bytes that would be freed.
    pub reclaimable_bytes: u64,
    /// How many targets that covers.
    pub target_count: usize,
}

/// A plain-language suggestion for the user.
///
/// Note what this type does *not* contain: no path, no command, nothing
/// actionable. Recommendations are explanations. Acting on one means the user
/// selecting targets from a `scan` and those going to `clean` — which is what
/// keeps the future AI layer (see `docs/ARCHITECTURE.md`) out of the delete
/// path entirely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    /// Short headline, e.g. `"npm cache is 2.1 GiB"`.
    pub title: String,
    /// The reasoning — why this is worth doing and what the cost is.
    pub detail: String,
    /// Space this recommendation would reclaim.
    pub reclaimable_bytes: u64,
    /// Care level of the targets behind it.
    pub risk: TargetRisk,
}

/// One target that could not be removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanFailure {
    /// The target's label, so the UI can name it without re-deriving it.
    pub label: String,
    /// Why it failed, in terms a user can act on.
    pub reason: String,
}

/// What actually happened during a clean.
///
/// Partial success is the normal case, not an exception: a permission error on
/// one directory shouldn't abandon the other nine. Both lists are reported so
/// the UI can be honest about it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanOutcome {
    /// Labels of targets successfully removed.
    pub removed: Vec<String>,
    /// Bytes actually reclaimed, measured — not the estimate.
    pub reclaimed_bytes: u64,
    /// Targets that could not be removed, and why.
    pub failures: Vec<CleanFailure>,
}

/// A [`CleanTarget`] that has passed both of the safety checks in
/// [`Plugin::clean`] and may therefore be removed.
///
/// This type is the safety contract, made structural. Its only field is
/// private, so no code outside this crate can construct one — which means the
/// only way a plugin ever receives something it is allowed to delete is if
/// [`Plugin::clean`] verified it first. A plugin author cannot forget the
/// checks, because there is no path to a deletable path that skips them.
///
/// Borrows rather than owns, so verification doesn't clone a list that may be
/// large and is discarded moments later.
///
/// The guarantee is enforced by the compiler, and this is the test of it —
/// building one outside this crate does not compile:
///
/// ```compile_fail
/// use sentinel_plugin_api::{CleanTarget, TargetRisk, VerifiedTarget};
///
/// let target = CleanTarget::new("npm", "/etc/passwd", "not a cache", 0, TargetRisk::Safe);
/// // `target` is private to the defining crate, so this cannot be written.
/// let forged = VerifiedTarget { target: &target };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedTarget<'a> {
    target: &'a CleanTarget,
}

impl<'a> VerifiedTarget<'a> {
    /// The path to remove.
    ///
    /// Confirmed to have been reported by this plugin's own `scan` moments
    /// ago. Note "moments": the filesystem can still change between that scan
    /// and the removal, so a plugin should use the same care it would with any
    /// path — this guarantees provenance, not exclusive access.
    pub fn path(&self) -> &Path {
        self.target.path()
    }

    /// What the user reviewed this target as.
    pub fn label(&self) -> &str {
        &self.target.label
    }

    /// Size recorded at scan time.
    pub fn size_bytes(&self) -> u64 {
        self.target.size_bytes
    }

    /// How much care this target needs.
    pub fn risk(&self) -> TargetRisk {
        self.target.risk
    }

    /// The underlying target, for reporting.
    pub fn target(&self) -> &'a CleanTarget {
        self.target
    }
}

/// Why a plugin operation didn't complete.
#[derive(Debug)]
pub enum PluginError {
    /// The tool this plugin manages isn't installed, or its cache directory
    /// doesn't exist. Expected and routine — not every machine has every tool.
    /// Callers should report it as "not applicable", not as a failure.
    ToolUnavailable {
        /// What was looked for and not found.
        detail: String,
    },
    /// A filesystem operation failed.
    Io(std::io::Error),
    /// A target was handed to the wrong plugin, or didn't come from a current
    /// scan. Refusing is mandatory — see the crate-level safety contract.
    RejectedTarget {
        /// Which target, and why it was refused.
        detail: String,
    },
    /// Anything else specific to the tool being managed.
    Other {
        /// Human-readable explanation.
        detail: String,
    },
}

impl fmt::Display for PluginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ToolUnavailable { detail } => write!(formatter, "tool unavailable: {detail}"),
            Self::Io(cause) => write!(formatter, "filesystem operation failed: {cause}"),
            Self::RejectedTarget { detail } => write!(formatter, "target refused: {detail}"),
            Self::Other { detail } => write!(formatter, "{detail}"),
        }
    }
}

impl Error for PluginError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(cause) => Some(cause),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PluginError {
    fn from(cause: std::io::Error) -> Self {
        Self::Io(cause)
    }
}

/// The contract every optimization target implements.
///
/// `Send + Sync` because the registry is shared across threads — the scheduler
/// runs scans off the UI thread, and Tauri may serve commands from a pool.
///
/// All methods are synchronous and may block; see the crate docs for why. Run
/// them somewhere that tolerates blocking.
pub trait Plugin: Send + Sync {
    /// Who this plugin is. Must return the same values every call.
    fn metadata(&self) -> PluginMetadata;

    /// Looks for removable data belonging to this plugin's tool.
    ///
    /// Must not modify anything. Returns [`PluginError::ToolUnavailable`] when
    /// the tool isn't installed, which callers treat as "not applicable"
    /// rather than as a failure.
    fn scan(&self) -> Result<ScanResult, PluginError>;

    /// How much space could be reclaimed.
    ///
    /// Defaults to totalling a fresh `scan`. Override only when the tool can
    /// answer more cheaply than a full walk.
    fn estimate(&self) -> Result<SpaceEstimate, PluginError> {
        let scan = self.scan()?;
        Ok(SpaceEstimate {
            reclaimable_bytes: scan.total_bytes(),
            target_count: scan.targets.len(),
        })
    }

    /// Explains, in plain language, what the user might want to do.
    ///
    /// Returns explanations only — never anything the caller can execute.
    fn recommend(&self) -> Result<Vec<Recommendation>, PluginError>;

    /// Removes the given targets, which have already been verified.
    ///
    /// This is the only method that deletes anything, and callers never invoke
    /// it directly — the free function [`clean`] is the entry point, and it
    /// only calls this with targets that passed both safety checks. A
    /// [`VerifiedTarget`] cannot be constructed outside this crate, so an
    /// implementation cannot be handed an unchecked path even by a caller
    /// trying to.
    ///
    /// Implementations must still:
    ///
    /// - Delete only [`VerifiedTarget::path`], exactly. No globs, no parent
    ///   directories, no string concatenation, no walking upward.
    /// - Report partial failure in [`CleanOutcome::failures`] rather than
    ///   aborting. A permission error on one directory should not abandon the
    ///   other nine.
    /// - Report `reclaimed_bytes` as what was actually freed, not
    ///   [`VerifiedTarget::size_bytes`], which is a scan-time estimate.
    fn remove_verified(&self, verified: &[VerifiedTarget<'_>])
        -> Result<CleanOutcome, PluginError>;
}

/// Rejects a tool directory that Sentinel should not go anywhere near.
///
/// Plugins locate their tool's cache by asking the tool, which means the answer
/// comes from a config file Sentinel didn't write and must be treated as
/// untrusted. This is the shared check, here rather than in each plugin so that
/// the twelfth plugin author can't quietly implement a weaker version of it.
///
/// Refuses a path that is relative, is a filesystem root, contains `..`, or
/// isn't an existing directory. `ToolUnavailable` rather than a failure,
/// because on most machines "that tool isn't set up here" is the ordinary
/// reason.
///
/// This is defence in depth, not the primary protection. The real guarantee is
/// that a plugin never proposes this directory itself — only known children of
/// it that already exist — so a path this implausible resolves to candidates
/// that don't exist. See `docs/PLUGIN_SDK.md`.
pub fn validate_tool_directory(candidate: &Path) -> Result<(), PluginError> {
    if !candidate.is_absolute() {
        return Err(PluginError::ToolUnavailable {
            detail: format!(
                "a relative cache path ({}) was reported, which Sentinel will not resolve",
                candidate.display()
            ),
        });
    }

    // A path with no parent is a filesystem root. Nothing legitimate puts a
    // package cache directly at `/` or `C:\`.
    if candidate.parent().is_none() {
        return Err(PluginError::ToolUnavailable {
            detail: format!(
                "a filesystem root ({}) was reported as a cache directory",
                candidate.display()
            ),
        });
    }

    // `..` means the path wasn't normalised, so comparing or joining it later
    // would not mean what it looks like it means.
    if candidate
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(PluginError::ToolUnavailable {
            detail: format!(
                "a cache path containing `..` ({}) was reported",
                candidate.display()
            ),
        });
    }

    if !candidate.is_dir() {
        return Err(PluginError::ToolUnavailable {
            detail: format!(
                "the cache directory ({}) does not exist yet",
                candidate.display()
            ),
        });
    }

    Ok(())
}

/// Removes exactly the targets given, and nothing else. **The only way to
/// delete anything in Sentinel.**
///
/// A free function rather than a method on [`Plugin`], deliberately. As a
/// provided trait method it could be overridden, and an override would skip
/// every check below — leaving the safety of the delete path resting on a
/// "don't override this" comment, which is the exact weakness this design
/// replaced. There is no `plugin.clean(..)` to override, so every caller goes
/// through here.
///
/// The checks, and why each one is here:
///
/// 1. **Every target must belong to this plugin.** A target carrying another
///    plugin's id means the caller has routed the list wrongly, so the whole
///    call fails with [`PluginError::RejectedTarget`] rather than proceeding
///    with a request that is already known to be confused.
/// 2. **Every path must appear in a scan taken right now.** The list was
///    reviewed by a human at some earlier moment and the filesystem may have
///    moved on. A target that no longer appears is refused individually and
///    reported in [`CleanOutcome::failures`] — it is usually just stale
///    (someone already cleared the cache), and failing the entire operation
///    over it would be unhelpful. It is also what stops a fabricated path from
///    ever reaching the filesystem.
///
/// Duplicate paths are collapsed, so a list naming the same thing twice can't
/// be double-counted in `reclaimed_bytes`.
///
/// An empty list returns an empty outcome without scanning — there is nothing
/// to verify and no reason to walk the filesystem to prove it.
///
/// Generic over `?Sized` so this works with `&dyn Plugin` out of a registry as
/// readily as with a concrete plugin.
pub fn clean<P>(plugin: &P, targets: &[CleanTarget]) -> Result<CleanOutcome, PluginError>
where
    P: Plugin + ?Sized,
{
    let id = plugin.metadata().id;

    // Checked before anything else, and fatal: unlike a stale path, a foreign
    // target is never a legitimate request.
    if let Some(foreign) = targets.iter().find(|target| target.plugin_id() != id) {
        return Err(PluginError::RejectedTarget {
            detail: format!(
                "target {:?} belongs to plugin {:?}, not {:?}",
                foreign.label,
                foreign.plugin_id(),
                id
            ),
        });
    }

    if targets.is_empty() {
        return Ok(CleanOutcome::default());
    }

    let current = plugin.scan()?;
    let allowed: HashSet<&Path> = current.targets.iter().map(CleanTarget::path).collect();

    let mut verified: Vec<VerifiedTarget<'_>> = Vec::with_capacity(targets.len());
    let mut refused = Vec::new();
    let mut seen: HashSet<&Path> = HashSet::with_capacity(targets.len());

    for target in targets {
        if !allowed.contains(target.path()) {
            refused.push(CleanFailure {
                label: target.label.clone(),
                reason: "no longer reported by a fresh scan, so it was not removed".to_owned(),
            });
            continue;
        }

        if seen.insert(target.path()) {
            verified.push(VerifiedTarget { target });
        }
    }

    if verified.is_empty() {
        return Ok(CleanOutcome {
            failures: refused,
            ..CleanOutcome::default()
        });
    }

    let mut outcome = plugin.remove_verified(&verified)?;
    outcome.failures.extend(refused);
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    #[test]
    fn total_bytes_sums_targets() {
        let scan = ScanResult {
            targets: vec![
                CleanTarget::new("npm", "/tmp/a", "a", 100, TargetRisk::Safe),
                CleanTarget::new("npm", "/tmp/b", "b", 250, TargetRisk::Review),
            ],
        };

        assert_eq!(scan.total_bytes(), 350);
    }

    #[test]
    fn total_bytes_saturates_instead_of_overflowing() {
        let scan = ScanResult {
            targets: vec![
                CleanTarget::new("npm", "/tmp/a", "a", u64::MAX, TargetRisk::Safe),
                CleanTarget::new("npm", "/tmp/b", "b", 1, TargetRisk::Safe),
            ],
        };

        assert_eq!(scan.total_bytes(), u64::MAX, "must not panic on overflow");
    }

    #[test]
    fn empty_scan_is_a_valid_success() {
        let scan = ScanResult::default();

        assert_eq!(scan.total_bytes(), 0);
        assert!(scan.targets.is_empty());
    }

    #[test]
    fn target_retains_its_provenance() {
        let target = CleanTarget::new(
            "npm",
            "/home/u/.npm/_cacache",
            "npm cache",
            42,
            TargetRisk::Safe,
        );

        assert_eq!(target.plugin_id(), "npm");
        assert_eq!(target.path(), Path::new("/home/u/.npm/_cacache"));
    }

    /// A plugin that records what it was asked to remove, so the tests can
    /// assert on what got past verification rather than on side effects.
    struct SpyPlugin {
        targets: Vec<CleanTarget>,
        scans: AtomicUsize,
        removed: Mutex<Vec<PathBuf>>,
    }

    impl SpyPlugin {
        fn with(paths: &[&str]) -> Self {
            Self {
                targets: paths
                    .iter()
                    .map(|path| CleanTarget::new("spy", *path, *path, 10, TargetRisk::Safe))
                    .collect(),
                scans: AtomicUsize::new(0),
                removed: Mutex::new(Vec::new()),
            }
        }

        fn removed_paths(&self) -> Vec<PathBuf> {
            self.removed.lock().expect("uncontended in tests").clone()
        }
    }

    impl Plugin for SpyPlugin {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata {
                id: "spy",
                name: "Spy",
                description: "Records what verification let through.",
            }
        }

        fn scan(&self) -> Result<ScanResult, PluginError> {
            self.scans.fetch_add(1, Ordering::SeqCst);
            Ok(ScanResult {
                targets: self.targets.clone(),
            })
        }

        fn recommend(&self) -> Result<Vec<Recommendation>, PluginError> {
            Ok(Vec::new())
        }

        fn remove_verified(
            &self,
            verified: &[VerifiedTarget<'_>],
        ) -> Result<CleanOutcome, PluginError> {
            let mut outcome = CleanOutcome::default();
            for target in verified {
                self.removed
                    .lock()
                    .expect("uncontended in tests")
                    .push(target.path().to_path_buf());
                outcome.removed.push(target.label().to_owned());
                outcome.reclaimed_bytes =
                    outcome.reclaimed_bytes.saturating_add(target.size_bytes());
            }
            Ok(outcome)
        }
    }

    #[test]
    fn clean_refuses_the_whole_call_for_a_foreign_target() {
        let plugin = SpyPlugin::with(&["/tmp/mine"]);
        let foreign = CleanTarget::new("other", "/tmp/theirs", "theirs", 10, TargetRisk::Safe);

        let error =
            clean(&plugin, &[foreign]).expect_err("a target from another plugin must be refused");

        assert!(
            matches!(error, PluginError::RejectedTarget { .. }),
            "got {error:?}"
        );
        assert!(
            plugin.removed_paths().is_empty(),
            "nothing may reach removal"
        );
    }

    /// The check that stops a fabricated path: a target carrying the right
    /// plugin id but a path this plugin never reported.
    #[test]
    fn clean_never_removes_a_path_absent_from_a_fresh_scan() {
        let plugin = SpyPlugin::with(&["/tmp/real"]);
        let forged = CleanTarget::new("spy", "/etc/passwd", "forged", 10, TargetRisk::Safe);

        let outcome = clean(&plugin, &[forged]).expect("refusal is not fatal");

        assert!(
            plugin.removed_paths().is_empty(),
            "a path the plugin never reported must never reach removal"
        );
        assert!(outcome.removed.is_empty());
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(outcome.reclaimed_bytes, 0);
    }

    #[test]
    fn one_stale_target_does_not_stop_the_valid_ones() {
        let plugin = SpyPlugin::with(&["/tmp/a", "/tmp/b"]);
        let scanned = plugin.scan().expect("scan succeeds");
        let stale = CleanTarget::new("spy", "/tmp/gone", "gone", 10, TargetRisk::Safe);

        let mut requested = scanned.targets.clone();
        requested.push(stale);

        let outcome = clean(&plugin, &requested).expect("partial success");

        assert_eq!(plugin.removed_paths().len(), 2);
        assert_eq!(outcome.removed.len(), 2);
        assert_eq!(outcome.failures.len(), 1);
    }

    #[test]
    fn duplicate_targets_are_collapsed() {
        let plugin = SpyPlugin::with(&["/tmp/a"]);
        let scanned = plugin.scan().expect("scan succeeds");
        let doubled = vec![scanned.targets[0].clone(), scanned.targets[0].clone()];

        let outcome = clean(&plugin, &doubled).expect("duplicates are tolerated");

        assert_eq!(
            plugin.removed_paths().len(),
            1,
            "the same path must not be removed twice"
        );
        assert_eq!(
            outcome.reclaimed_bytes, 10,
            "space must not be double-counted"
        );
    }

    #[test]
    fn cleaning_nothing_does_not_touch_the_filesystem() {
        let plugin = SpyPlugin::with(&["/tmp/a"]);

        let outcome = clean(&plugin, &[]).expect("an empty clean succeeds");

        assert_eq!(outcome, CleanOutcome::default());
        assert_eq!(
            plugin.scans.load(Ordering::SeqCst),
            0,
            "an empty list has nothing to verify, so it should not scan"
        );
    }

    #[test]
    fn verification_scans_once_per_clean() {
        let plugin = SpyPlugin::with(&["/tmp/a", "/tmp/b"]);
        let scanned = plugin.scan().expect("scan succeeds");
        plugin.scans.store(0, Ordering::SeqCst);

        clean(&plugin, &scanned.targets).expect("clean succeeds");

        assert_eq!(
            plugin.scans.load(Ordering::SeqCst),
            1,
            "verification should re-scan once, not once per target"
        );
    }

    #[test]
    fn destructive_targets_demand_a_typed_confirmation() {
        assert!(TargetRisk::Destructive.requires_typed_confirmation());
        assert!(!TargetRisk::Review.requires_typed_confirmation());
        assert!(!TargetRisk::Safe.requires_typed_confirmation());

        assert!(TargetRisk::Safe.may_be_preselected());
        assert!(
            !TargetRisk::Review.may_be_preselected(),
            "a pre-ticked box is not a decision"
        );
        assert!(!TargetRisk::Destructive.may_be_preselected());
    }

    #[test]
    fn risk_serializes_for_the_frontend() {
        let json = serde_json::to_string(&TargetRisk::Destructive).expect("plain data");
        assert_eq!(json, "\"destructive\"");
    }

    #[test]
    fn tool_unavailable_reads_as_a_normal_condition() {
        let error = PluginError::ToolUnavailable {
            detail: "npm is not installed".to_owned(),
        };

        assert_eq!(error.to_string(), "tool unavailable: npm is not installed");
    }
}
