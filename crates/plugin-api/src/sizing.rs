//! Measuring how much space a directory would actually free.
//!
//! This lives here, rather than in each plugin, because the number it produces
//! is a promise to the user and getting it right is subtle in several ways that
//! are easy to miss. Twelve plugins each writing their own walk would mean
//! twelve chances to overstate what a clean will give back — and the npm plugin
//! overstated its `_npx` directory by 10.6 MiB before the hardlink handling
//! below existed. One implementation means that fix can't be forgotten by the
//! seventh plugin author.
//!
//! # Why this counts allocated blocks, not file lengths
//!
//! This measured **apparent size** (`metadata.len()`, matching `du -sb`) until
//! it was checked against the real caches on a developer machine. That was
//! wrong in both directions at once, which is why it changed:
//!
//! - **It over-promised on sparse files.** Chromium writes them everywhere. VS
//!   Code's `GPUCache/data_3` is 4,202,496 bytes long but occupies 196,608 —
//!   a 21× over-statement on one file, and a whole `GPUCache` that claimed
//!   8.6 MiB would have freed 4.1 MiB.
//! - **It under-reported caches of many small files**, which is most of them: a
//!   100-byte file still occupies a full block. npm's `_npx` read 17% low,
//!   cargo's `registry/src` 24% low.
//! - **It hid purged caches on Linux.** A directory skeleton with no files left
//!   measured zero, so callers skipped it — pip's `http-v2` was 9.9 MiB of
//!   reclaimable blocks in 2,525 empty directories that Sentinel never offered.
//!
//! Apparent size was never the conservative choice it looked like. Allocated
//! blocks answer the question actually being asked — *how much space do I get
//! back* — and match plain `du`, which is what a user checks against. The
//! decision is recorded in `docs/ARCHITECTURE.md`.

use std::fs;
use std::path::Path;

use crate::PluginError;

/// How many bytes removing a directory would actually free.
///
/// Counts **allocated blocks**, so the figure matches plain `du` rather than
/// `du -sb` — see the module docs for why that changed. Five behaviours, each
/// deliberate:
///
/// - **Directories count where the filesystem reports them.** On Linux/ext4 a
///   directory commonly occupies 4 KiB of its own, and `remove_dir_all` frees
///   it. APFS reports zero `st_blocks` for empty directories, matching macOS
///   `du`; Sentinel accepts that conservative zero instead of inventing an
///   allocation the OS does not expose.
/// - **Symlinks are counted as the link, never followed.** Following one would
///   inflate the figure with data that lives elsewhere and would not be freed
///   by removing this directory.
/// - **Hardlinked files are counted once.** Package managers hardlink
///   aggressively; a file with three links in the tree occupies its blocks
///   once, so counting it three times overstates the reclaimable space. This
///   matches `du`'s default, which is what a user will check against.
/// - **Unreadable entries are skipped rather than fatal.** A cache with one
///   unreadable file is still worth reporting, and the result is then an
///   *under*estimate — the right direction to be wrong in when the number is a
///   promise. The root not being readable is still an error, since that means
///   the whole measurement is meaningless.
/// - **Windows falls back to apparent size.** `std::fs::Metadata` exposes no
///   block count there, so the figure carries the sparse-file and small-file
///   inaccuracies described in the module docs. Documented rather than papered
///   over, exactly like the hardlink caveat on [`HardLinkSet`].
///
/// Walks iteratively rather than recursively: package caches nest deeply
/// (npm's `_cacache` especially), and recursion would put that depth on the
/// stack for no benefit.
pub fn directory_size(root: &Path) -> Result<u64, PluginError> {
    let mut total: u64 = 0;
    let mut pending = vec![root.to_path_buf()];
    // Only files with more than one link go in here, so the common case costs
    // nothing — a cache may hold tens of thousands of files and almost none of
    // them are linked twice.
    let mut counted_links = HardLinkSet::default();

    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            // A subdirectory vanishing mid-walk is normal — the tool that owns
            // this cache is allowed to be running at the same time.
            Err(cause) if directory == root => return Err(PluginError::Io(cause)),
            Err(_) => continue,
        };

        // The directory's own blocks, charged once as it is visited. `root`
        // reaches this branch too, so the total includes the directory the
        // caller asked about, which `remove_dir_all` also removes.
        //
        // `symlink_metadata` rather than `metadata`: this path came from a
        // parent's `read_dir` and was confirmed to be a directory there, but
        // asking without following keeps the "never follow a link" rule true
        // at every step rather than only at the leaves.
        if let Ok(metadata) = fs::symlink_metadata(&directory) {
            total = total.saturating_add(allocated_size(&metadata));
        }

        for entry in entries.flatten() {
            // `DirEntry::metadata` does not follow symlinks. The name doesn't
            // say so, and this feeds a file-deleting code path, so it's worth
            // stating rather than leaving a reader to remember it.
            let Ok(metadata) = entry.metadata() else {
                continue;
            };

            if metadata.is_dir() {
                pending.push(entry.path());
            } else if counted_links.is_first_sighting(&metadata) {
                total = total.saturating_add(allocated_size(&metadata));
            }
        }
    }

    Ok(total)
}

/// How many bytes removing a single file would actually free.
///
/// The file counterpart of [`directory_size`], and it exists for the same
/// reason: a plugin that proposes individual files rather than whole
/// directories still owes the user a figure measured the same way as every
/// other figure in the app. Reaching for `metadata.len()` instead would put an
/// apparent-size number next to allocated-size numbers on the same screen,
/// which is exactly the inconsistency the module docs describe.
///
/// Never follows symlinks — the size reported is the link's own, since removing
/// a link frees only the link.
///
/// Takes a path rather than `Metadata` so callers can't accidentally pass
/// metadata they obtained by following a link.
pub fn file_size(path: &Path) -> Result<u64, PluginError> {
    let metadata = fs::symlink_metadata(path).map_err(PluginError::Io)?;

    Ok(allocated_size(&metadata))
}

/// Whether a directory holds nothing at all.
///
/// Plugins use this to decide whether a target is worth offering. It used to be
/// expressed as `directory_size(path) == 0`, which stopped meaning anything
/// once sizing counted blocks: every directory that exists commonly occupies
/// its own block on Linux, so a Bun cache that had never been used would have
/// been offered despite containing nothing.
///
/// The two questions were only ever conflated by accident, and they come apart
/// in exactly the case that motivated the sizing change — pip's `http-v2` holds
/// 2,525 empty directories, so it is *not* empty and its 9.9 MiB is real, while
/// a freshly created cache root holds nothing and has nothing to give back.
///
/// An unreadable directory reads as non-empty: refusing to offer a target
/// because it couldn't be inspected would hide it silently, and the size walk
/// already tolerates unreadable entries.
pub fn is_empty_directory(path: &Path) -> bool {
    fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_none())
}

/// The space this entry occupies on disk, as opposed to how long it is.
///
/// The two differ constantly and in both directions: a 100-byte file still
/// takes a whole block, while a sparse file reports a length far larger than
/// the blocks behind it. Only the block count says what removing it frees.
#[cfg(unix)]
fn allocated_size(metadata: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;

    // `st_blocks` is defined in 512-byte units regardless of the filesystem's
    // own block size — that is the unit `du` reports in by default, not a
    // guess about ext4.
    metadata.blocks().saturating_mul(512)
}

/// The space this entry occupies on disk.
///
/// Windows exposes no block count through `std::fs::Metadata`, so this is the
/// apparent size and inherits the inaccuracies the module docs describe. A
/// correct figure needs `GetFileInformationByHandleEx` with an open handle,
/// which is a real piece of work rather than a missing method call.
#[cfg(not(unix))]
fn allocated_size(metadata: &fs::Metadata) -> u64 {
    metadata.len()
}

/// Remembers which multiply-linked files have already been counted.
///
/// Unix only. Windows exposes no equivalent identity through `fs::metadata` —
/// `file_index` needs an open handle — so there every link is counted and the
/// figure can overstate on a volume that uses hardlinks. Documented rather than
/// papered over.
#[derive(Default)]
struct HardLinkSet {
    #[cfg(unix)]
    seen: std::collections::HashSet<(u64, u64)>,
}

impl HardLinkSet {
    /// Whether this file's blocks have not yet been counted.
    #[cfg(unix)]
    fn is_first_sighting(&mut self, metadata: &fs::Metadata) -> bool {
        use std::os::unix::fs::MetadataExt;

        if metadata.nlink() <= 1 {
            return true;
        }

        self.seen.insert((metadata.dev(), metadata.ino()))
    }

    /// Whether this file's blocks have not yet been counted.
    #[cfg(not(unix))]
    fn is_first_sighting(&mut self, _metadata: &fs::Metadata) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(files: &[(&str, usize)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        for (name, size) in files {
            let path = dir.path().join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            fs::write(&path, vec![b'x'; *size]).expect("write file");
        }
        dir
    }

    /// Sizes are asserted as properties rather than exact byte counts. A
    /// block-counting walk depends on the filesystem under `/tmp` — block size,
    /// whether it sparsifies, tail packing — and pinning a literal would be
    /// testing this machine rather than the function.
    #[test]
    fn counts_every_file_across_nested_directories() {
        let dir = dir_with(&[("a", 100), ("nested/b", 250), ("nested/deep/c", 150)]);

        let measured = directory_size(dir.path()).expect("walk succeeds");

        assert!(
            measured >= 500,
            "three files totalling 500 bytes occupy at least 500 bytes, got {measured}"
        );
        assert_eq!(measured % 512, 0, "block counts are 512-byte units");
    }

    /// Small files round up to a whole block, so the space reclaimed is more
    /// than the bytes written. Getting this wrong made npm's `_npx` read 17%
    /// low and cargo's `registry/src` 24% low.
    #[cfg(unix)]
    #[test]
    fn a_small_file_costs_a_whole_block() {
        let dir = dir_with(&[("tiny", 1)]);

        assert!(
            directory_size(dir.path()).expect("walk succeeds") > 1,
            "a one-byte file still occupies a block, and removing it frees that block"
        );
    }

    /// The change that unhid pip's `http-v2`: a purged cache is a tree of empty
    /// directories, each occupying blocks that `remove_dir_all` gives back.
    /// Measuring only file lengths reported 9.9 MiB of real space as zero, and
    /// callers skip a zero-sized target.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_skeleton_of_empty_directories_is_not_zero() {
        let dir = tempfile::tempdir().expect("temp dir");
        for index in 0..32 {
            fs::create_dir_all(dir.path().join(format!("shard-{index}/nested")))
                .expect("create directory");
        }

        assert!(
            directory_size(dir.path()).expect("walk succeeds") > 0,
            "empty directories still occupy space, and a purged cache is worth offering"
        );
    }

    /// A sparse file is *shorter on disk than it is long*. Chromium writes them
    /// throughout its caches: VS Code's `GPUCache/data_3` is 4,202,496 bytes
    /// long and occupies 196,608. Measuring length promised 21× what a clean
    /// would deliver, which is the failure this function's rewrite exists for.
    #[cfg(unix)]
    #[test]
    fn a_sparse_file_is_measured_by_what_it_occupies() {
        use std::os::unix::fs::MetadataExt;

        const LENGTH: u64 = 8 * 1024 * 1024;

        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("sparse");
        let file = fs::File::create(&path).expect("create file");
        file.set_len(LENGTH).expect("extend without writing");
        drop(file);

        let metadata = fs::metadata(&path).expect("stat");
        if metadata.blocks() * 512 >= LENGTH {
            // Some filesystems allocate eagerly. Nothing to assert here then —
            // reported rather than silently passing, so a green run on such a
            // machine isn't mistaken for coverage.
            eprintln!("skipped: {} does not create sparse files", path.display());
            return;
        }

        assert!(
            directory_size(dir.path()).expect("walk succeeds") < LENGTH,
            "a sparse file must be measured by its blocks, never by its length"
        );
    }

    #[test]
    fn a_directory_with_nothing_in_it_reads_as_empty() {
        let dir = tempfile::tempdir().expect("temp dir");

        assert!(is_empty_directory(dir.path()));
    }

    /// The distinction the sizing change turns on: a purged cache is a tree of
    /// empty directories, which is *not* an empty directory even on a
    /// filesystem such as APFS that reports zero allocated blocks for it.
    #[test]
    fn a_tree_of_empty_directories_is_not_itself_empty() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(dir.path().join("shard/nested")).expect("create directory");

        assert!(!is_empty_directory(dir.path()));
    }

    #[test]
    fn an_unreadable_directory_does_not_read_as_empty() {
        assert!(
            !is_empty_directory(Path::new("/nonexistent/sentinel-sizing-test")),
            "a target must not be dropped silently because it couldn't be inspected"
        );
    }

    #[test]
    fn a_missing_root_is_an_error() {
        let error = directory_size(Path::new("/nonexistent/sentinel-sizing-test"))
            .expect_err("a missing root cannot be measured");

        assert!(matches!(error, PluginError::Io(_)), "got {error:?}");
    }

    /// The promise this function makes: removing the tree frees these bytes.
    /// Two links to one file free its blocks once.
    ///
    /// Asserted by comparing against the same tree without the extra link,
    /// rather than against a literal — the property is "the link is free", and
    /// that holds whatever the block size turns out to be.
    #[cfg(unix)]
    #[test]
    fn a_hardlinked_file_is_counted_once() {
        let without = dir_with(&[("original", 1000)]);
        let with = dir_with(&[("original", 1000)]);
        fs::hard_link(with.path().join("original"), with.path().join("link")).expect("hard link");

        assert_eq!(
            directory_size(with.path()).expect("walk succeeds"),
            directory_size(without.path()).expect("walk succeeds"),
            "a second link to the same blocks must add nothing"
        );
    }

    /// Removing a directory does not remove what its symlinks point at, so the
    /// figure must not include it. Asserted against the identical tree without
    /// the link: whatever a link costs, it is not the 100 KB on the other end.
    #[cfg(unix)]
    #[test]
    fn a_symlink_is_not_followed() {
        let without = dir_with(&[("small", 10)]);
        let with = dir_with(&[("small", 10)]);
        let outside = dir_with(&[("big", 100_000)]);
        std::os::unix::fs::symlink(outside.path(), with.path().join("elsewhere"))
            .expect("create symlink");

        let with_link = directory_size(with.path()).expect("walk succeeds");
        let without_link = directory_size(without.path()).expect("walk succeeds");

        assert!(
            with_link < without_link + 100_000,
            "a symlink must not pull in data that removing this directory wouldn't free \
             ({with_link} vs {without_link} without the link)"
        );
    }
}
