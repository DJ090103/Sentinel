//! Read-only analysis of the user's home folder: where the space went, the
//! largest files, and exact duplicates.
//!
//! This exists apart from the plugins because it never proposes a deletion.
//! A plugin's job is to name things Sentinel may remove; these are the user's
//! own files, so the output is a report and nothing here can reach a `clean()`.
//!
//! One walk feeds all three results. Hidden entries (caches, app data, `.git`),
//! symlinks and build output (`node_modules`, `target`) are skipped: hidden
//! data is the plugins' territory, links would double-count or escape the home
//! folder, and build output is regenerable noise in a duplicate list.

use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use sentinel_plugin_api::file_size;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Files smaller than this are not considered for duplicate detection; the
/// wasted space is not worth hashing for and small files duplicate by design.
pub const MIN_DUPLICATE_BYTES: u64 = 1024 * 1024;
/// Stop walking after this many files, and say so in the report.
pub const MAX_FILES: u64 = 3_000_000;
const TOP_FOLDERS: usize = 15;
const TOP_FILES: usize = 25;
const TOP_DUPLICATE_GROUPS: usize = 20;
const PARTIAL_HASH_BYTES: u64 = 64 * 1024;
const PROGRESS_EVERY: u64 = 2_000;
const SKIPPED_DIRECTORIES: &[&str] = &["node_modules", "target", "AppData"];

/// Which part of the analysis is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    /// Walking the folder tree.
    Walking,
    /// Hashing same-sized files to find exact duplicates.
    Duplicates,
}

/// A progress reading, for the UI's staged progress display.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// Current stage.
    pub stage: Stage,
    /// Files seen so far (walking) or candidate files hashed (duplicates).
    pub files: u64,
    /// Bytes seen so far (walking) or hashed (duplicates).
    pub bytes: u64,
}

/// A top-level folder of the home directory and how much it holds.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FolderUsage {
    /// Absolute path.
    pub path: PathBuf,
    /// Allocated bytes of everything beneath it that the walk counted.
    pub size_bytes: u64,
}

/// One large file.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LargeFile {
    /// Absolute path.
    pub path: PathBuf,
    /// Allocated bytes.
    pub size_bytes: u64,
}

/// Files with identical content.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    /// Size of each copy.
    pub size_bytes: u64,
    /// Every copy, sorted. At least two.
    pub paths: Vec<PathBuf>,
}

impl DuplicateGroup {
    /// Space freed by keeping one copy.
    pub fn wasted_bytes(&self) -> u64 {
        self.size_bytes * (self.paths.len() as u64 - 1)
    }
}

/// Everything the analysis found.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HomeAnalysis {
    /// Largest top-level folders, biggest first.
    pub folders: Vec<FolderUsage>,
    /// Largest files, biggest first.
    pub largest_files: Vec<LargeFile>,
    /// Duplicate groups, most wasted space first.
    pub duplicates: Vec<DuplicateGroup>,
    /// Files counted.
    pub total_files: u64,
    /// Allocated bytes counted.
    pub total_bytes: u64,
    /// True if [`MAX_FILES`] stopped the walk early.
    pub truncated: bool,
}

/// Why an analysis produced no report.
#[derive(Debug)]
pub enum AnalysisError {
    /// The user cancelled.
    Cancelled,
    /// The root could not be read at all.
    Unreadable(io::Error),
}

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "the analysis was cancelled"),
            Self::Unreadable(e) => write!(f, "the home folder could not be read: {e}"),
        }
    }
}

impl std::error::Error for AnalysisError {}

/// Walks `root` once and returns folder usage, largest files and duplicates.
///
/// `cancel` is polled between directories and files; `on_progress` is called
/// periodically so a caller can show live stages without core knowing about
/// any UI.
pub fn analyze_home(
    root: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<HomeAnalysis, AnalysisError> {
    let top_level = fs::read_dir(root).map_err(AnalysisError::Unreadable)?;

    let mut walk = Walk::default();
    let mut truncated = false;

    // (directory, the top-level folder it belongs to, if any)
    let mut pending: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();

    for entry in top_level.flatten() {
        if let Some((path, is_dir)) = classify(&entry) {
            walk.accept(path, is_dir, None, &mut pending, on_progress);
        }
    }

    while let Some((directory, top)) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Err(AnalysisError::Cancelled);
        }
        if walk.total_files >= MAX_FILES {
            truncated = true;
            break;
        }
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            if let Some((path, is_dir)) = classify(&entry) {
                walk.accept(path, is_dir, top.clone(), &mut pending, on_progress);
            }
        }
    }

    let Walk {
        folder_sizes,
        largest,
        by_size,
        total_files,
        total_bytes,
        ..
    } = walk;

    on_progress(Progress {
        stage: Stage::Walking,
        files: total_files,
        bytes: total_bytes,
    });

    let duplicates = find_duplicates(by_size, cancel, on_progress)?;

    let mut folders: Vec<FolderUsage> = folder_sizes
        .into_iter()
        .filter(|(_, size)| *size > 0)
        .map(|(path, size_bytes)| FolderUsage { path, size_bytes })
        .collect();
    folders.sort_by(|a, b| {
        b.size_bytes
            .cmp(&a.size_bytes)
            .then_with(|| a.path.cmp(&b.path))
    });
    folders.truncate(TOP_FOLDERS);

    let mut largest_files: Vec<LargeFile> = largest
        .into_iter()
        .map(|std::cmp::Reverse((size_bytes, path))| LargeFile { path, size_bytes })
        .collect();
    largest_files.sort_by(|a, b| {
        b.size_bytes
            .cmp(&a.size_bytes)
            .then_with(|| a.path.cmp(&b.path))
    });

    Ok(HomeAnalysis {
        folders,
        largest_files,
        duplicates,
        total_files,
        total_bytes,
        truncated,
    })
}

/// Accumulated state of the single walk.
#[derive(Default)]
struct Walk {
    folder_sizes: HashMap<PathBuf, u64>,
    largest: BinaryHeap<std::cmp::Reverse<(u64, PathBuf)>>,
    by_size: HashMap<u64, Vec<PathBuf>>,
    seen_inodes: HashSet<(u64, u64)>,
    total_files: u64,
    total_bytes: u64,
}

type Pending = Vec<(PathBuf, Option<PathBuf>)>;

impl Walk {
    /// Queues a directory or counts a file.
    fn accept(
        &mut self,
        path: PathBuf,
        is_dir: bool,
        top: Option<PathBuf>,
        pending: &mut Pending,
        on_progress: &mut dyn FnMut(Progress),
    ) {
        if is_dir {
            // A top-level directory is its own bucket.
            let bucket = top.unwrap_or_else(|| path.clone());
            self.folder_sizes.entry(bucket.clone()).or_insert(0);
            pending.push((path, Some(bucket)));
            return;
        }
        let Ok(size) = file_size(&path) else { return };
        self.total_files += 1;
        self.total_bytes = self.total_bytes.saturating_add(size);
        if let Some(bucket) = top {
            *self.folder_sizes.entry(bucket).or_insert(0) += size;
        }
        self.largest.push(std::cmp::Reverse((size, path.clone())));
        if self.largest.len() > TOP_FILES {
            self.largest.pop();
        }
        if size >= MIN_DUPLICATE_BYTES && !is_extra_hard_link(&path, &mut self.seen_inodes) {
            self.by_size.entry(size).or_default().push(path);
        }
        if self.total_files.is_multiple_of(PROGRESS_EVERY) {
            on_progress(Progress {
                stage: Stage::Walking,
                files: self.total_files,
                bytes: self.total_bytes,
            });
        }
    }
}

/// Decides whether one directory entry is walked, counted, or ignored.
/// Returns the path and whether it is a directory.
fn classify(entry: &fs::DirEntry) -> Option<(PathBuf, bool)> {
    let name = entry.file_name();
    let name = name.to_string_lossy();
    if name.starts_with('.') {
        return None;
    }
    // `DirEntry::file_type` does not follow symlinks.
    let file_type = entry.file_type().ok()?;
    if file_type.is_dir() {
        (!SKIPPED_DIRECTORIES.contains(&name.as_ref())).then(|| (entry.path(), true))
    } else {
        file_type.is_file().then(|| (entry.path(), false))
    }
}

/// True for the second and later names of a hard-linked file: they are one
/// copy on disk, so reporting them as duplicates would promise space that
/// deleting them cannot free.
#[cfg(unix)]
fn is_extra_hard_link(path: &Path, seen: &mut HashSet<(u64, u64)>) -> bool {
    use std::os::unix::fs::MetadataExt;
    match fs::symlink_metadata(path) {
        Ok(m) if m.nlink() > 1 => !seen.insert((m.dev(), m.ino())),
        _ => false,
    }
}

#[cfg(not(unix))]
fn is_extra_hard_link(_path: &Path, _seen: &mut HashSet<(u64, u64)>) -> bool {
    false
}

/// Narrows same-size files to identical ones: first 64 KiB, then full SHA-256.
fn find_duplicates(
    by_size: HashMap<u64, Vec<PathBuf>>,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<Vec<DuplicateGroup>, AnalysisError> {
    let (mut hashed_files, mut hashed_bytes) = (0u64, 0u64);
    let mut groups = Vec::new();

    for (size, paths) in by_size.into_iter().filter(|(_, p)| p.len() > 1) {
        if cancel.load(Ordering::Relaxed) {
            return Err(AnalysisError::Cancelled);
        }
        let mut partial: HashMap<[u8; 32], Vec<PathBuf>> = HashMap::new();
        for path in paths {
            if let Ok(hash) = hash_file(&path, Some(PARTIAL_HASH_BYTES)) {
                partial.entry(hash).or_default().push(path);
            }
        }
        for candidates in partial.into_values().filter(|c| c.len() > 1) {
            let mut full: HashMap<[u8; 32], Vec<PathBuf>> = HashMap::new();
            for path in candidates {
                if let Ok(hash) = hash_file(&path, None) {
                    hashed_files += 1;
                    hashed_bytes = hashed_bytes.saturating_add(size);
                    full.entry(hash).or_default().push(path);
                }
            }
            for mut paths in full.into_values().filter(|p| p.len() > 1) {
                paths.sort();
                groups.push(DuplicateGroup {
                    size_bytes: size,
                    paths,
                });
            }
        }
        on_progress(Progress {
            stage: Stage::Duplicates,
            files: hashed_files,
            bytes: hashed_bytes,
        });
    }

    groups.sort_by(|a, b| {
        b.wasted_bytes()
            .cmp(&a.wasted_bytes())
            .then_with(|| a.paths.cmp(&b.paths))
    });
    groups.truncate(TOP_DUPLICATE_GROUPS);
    Ok(groups)
}

/// SHA-256 of a file, or of its first `limit` bytes.
fn hash_file(path: &Path, limit: Option<u64>) -> io::Result<[u8; 32]> {
    let file = File::open(path)?;
    let mut reader: Box<dyn Read> = match limit {
        Some(limit) => Box::new(file.take(limit)),
        None => Box::new(file),
    };
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, byte: u8, len: usize) {
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, vec![byte; len]).expect("write");
    }

    #[test]
    fn finds_folders_large_files_and_exact_duplicates_only() {
        let dir = tempfile::tempdir().expect("temp dir");
        let home = dir.path();
        let mib = 1024 * 1024;
        write(&home.join("Docs/a.bin"), 1, 2 * mib);
        write(&home.join("Downloads/a-copy.bin"), 1, 2 * mib);
        // Same size, different content: must not be reported.
        write(&home.join("Downloads/other.bin"), 2, 2 * mib);
        // Skipped: hidden, build output.
        write(&home.join(".cache/big.bin"), 3, 4 * mib);
        write(&home.join("proj/node_modules/x.bin"), 1, 2 * mib);

        let analysis =
            analyze_home(home, &AtomicBool::new(false), &mut |_| {}).expect("analysis succeeds");

        assert_eq!(analysis.duplicates.len(), 1);
        assert_eq!(analysis.duplicates[0].paths.len(), 2);
        assert!(analysis.duplicates[0]
            .paths
            .iter()
            .all(
                |p| p.extension().is_some_and(|e| e == "bin") && !p.starts_with(home.join("proj"))
            ));
        assert_eq!(analysis.total_files, 3);
        assert!(analysis
            .largest_files
            .iter()
            .all(|f| !f.path.starts_with(home.join(".cache"))));
        assert!(!analysis.truncated);
        assert_eq!(analysis.folders.len(), 2);
    }

    #[test]
    fn cancelling_stops_the_walk() {
        let dir = tempfile::tempdir().expect("temp dir");
        write(&dir.path().join("a/f"), 1, 10);
        let result = analyze_home(dir.path(), &AtomicBool::new(true), &mut |_| {});
        assert!(matches!(result, Err(AnalysisError::Cancelled)));
    }
}
