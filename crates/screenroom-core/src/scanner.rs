//! Bounded, cancellable, non-destructive media scanning.
//!
//! Guarantees:
//!
//! * Traversal never follows a symbolic link, never leaves the authorized root,
//!   and stops at an entry cap and a depth cap rather than running away.
//! * A file still being written (young mtime, zero size, or a download sidecar)
//!   is reported and skipped, not imported in half-written form.
//! * Failures are per-entry [`ScanIssue`] values; one unreadable folder does not
//!   abort the walk, and the scan reports whether results are complete.
//! * Only a complete scan may withdraw availability, so a cancelled or bounded
//!   scan can never make the library look empty.
//! * Nothing on disk is modified, moved or deleted.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use crate::domain::SourceState;
use crate::error::{CoreError, Result};
use crate::filename::parse_filename;
use crate::fingerprint::media_fingerprint;
use crate::library::{Ingest, Library};

/// Container extensions the library will import.
///
/// A container is not a codec promise: playback compatibility is decided by the
/// platform player for the actual stream, never by this list.
pub const MEDIA_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "m4v", "avi", "mov", "webm", "mpeg", "mpg", "ts", "m2ts", "wmv", "flv", "ogv",
    "3gp",
];

/// Suffixes that indicate a download is still in progress.
const DOWNLOAD_SUFFIXES: &[&str] = &[".part", ".crdownload", ".download", ".aria2", ".!qB"];

/// A cooperative cancellation flag shared with the scanning thread.
#[derive(Debug, Clone, Default)]
pub struct ScanToken(Arc<AtomicBool>);

impl ScanToken {
    /// A fresh, uncancelled token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks the running scan to stop at the next safe point.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// True once cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// One entry the scan could not use, with a stable code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanIssue {
    /// Path relative to the source root, empty for source-wide issues.
    pub relative_path: String,
    /// Stable reason code, for example `still_downloading`.
    pub code: &'static str,
}

/// Progress and partial results for one step of a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanUpdate {
    /// Entries examined so far.
    pub visited: u64,
    /// Files imported or refreshed so far.
    pub imported: u64,
    /// Issues seen so far.
    pub errors: Vec<ScanIssue>,
    /// True for the final update.
    pub done: bool,
    /// True when the final update was caused by cancellation.
    pub cancelled: bool,
}

/// Final outcome of a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanSummary {
    /// Entries examined.
    pub visited: u64,
    /// Files imported or refreshed.
    pub imported: u64,
    /// Issues seen.
    pub issues: Vec<ScanIssue>,
    /// Whether cancellation was observed.
    pub cancelled: bool,
    /// Whether the walk saw every entry and raised no issue.
    ///
    /// Only a complete scan may withdraw availability from missing files.
    pub complete: bool,
}

/// Walks one authorized source and records what it finds.
#[derive(Debug)]
pub struct LibraryScanner<'a> {
    library: &'a Library,
    max_entries: u64,
    max_depth: usize,
    stable_age: Duration,
    max_issues: usize,
}

impl<'a> LibraryScanner<'a> {
    /// A scanner with production bounds: 250k entries, depth 64, 15s settle time.
    pub fn new(library: &'a Library) -> Self {
        Self {
            library,
            max_entries: 250_000,
            max_depth: 64,
            stable_age: Duration::from_secs(15),
            max_issues: 1_000,
        }
    }

    /// Replaces the traversal and settle bounds, mainly for tests and small devices.
    pub fn with_limits(mut self, max_entries: u64, max_depth: usize, stable_age: Duration) -> Self {
        self.max_entries = max_entries;
        self.max_depth = max_depth;
        self.stable_age = stable_age;
        self
    }

    /// Scans `source_id`, invoking `on_update` as files are imported.
    ///
    /// The callback runs on the scanning thread; it must not call back into the
    /// library for writes.
    pub fn scan(
        &self,
        source_id: &str,
        token: &ScanToken,
        mut on_update: impl FnMut(&ScanUpdate),
    ) -> Result<ScanSummary> {
        self.library.claim_scan(source_id)?;
        let _guard = ScanGuard {
            library: self.library,
            source_id: source_id.to_string(),
        };

        let source = self.library.source(source_id)?;
        if source.state == SourceState::Removed {
            return Err(CoreError::library(
                "source_removed",
                "This source was removed.",
            ));
        }
        if source.root_path.is_empty() {
            return Err(CoreError::library(
                "source_disconnected",
                "Relink the source before scanning.",
            ));
        }

        let mut issues = Issues::new(self.max_issues);
        let root = match std::fs::canonicalize(&source.root_path) {
            Ok(root) => root,
            Err(error) => {
                let (state, code) = unavailable_root(Path::new(&source.root_path), error.kind());
                self.library.set_source_state(source_id, state)?;
                issues.push("", code);
                let update = ScanUpdate {
                    visited: 0,
                    imported: 0,
                    errors: issues.snapshot(),
                    done: true,
                    cancelled: false,
                };
                on_update(&update);
                return Ok(ScanSummary {
                    visited: 0,
                    imported: 0,
                    issues: issues.into_vec(),
                    cancelled: false,
                    complete: false,
                });
            }
        };
        // Compared as locations, not as strings. `canonicalize` does not return the
        // spelling it was given — on Windows it answers with a verbatim path
        // (`\\?\D:\Media` for `D:\Media`) — so a literal comparison rejected every
        // healthy Windows source as a replaced symbolic link.
        let stored_key = self.library.path_key(Path::new(&source.root_path));
        if self.library.path_key(&root) != stored_key {
            return Err(CoreError::library(
                "unsafe_path",
                "The source was replaced by a symbolic link. Relink it explicitly.",
            ));
        }

        let scan_id = crate::library::new_id();
        let mut visited = 0_u64;
        let mut imported = 0_u64;
        let mut bounded = false;
        let mut queue = VecDeque::from([(root.clone(), 0_usize)]);

        while let Some((directory, depth)) = queue.pop_front() {
            if token.is_cancelled() {
                break;
            }
            let entries = match std::fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) => {
                    issues.push_at(
                        &directory,
                        &root,
                        if error.kind() == std::io::ErrorKind::PermissionDenied {
                            "permission_lost"
                        } else {
                            "directory_unavailable"
                        },
                    );
                    continue;
                }
            };
            for entry in entries {
                if token.is_cancelled() {
                    break;
                }
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) => {
                        issues.push_at(
                            &directory,
                            &root,
                            if error.kind() == std::io::ErrorKind::PermissionDenied {
                                "permission_lost"
                            } else {
                                "read_failed"
                            },
                        );
                        continue;
                    }
                };
                visited += 1;
                if visited > self.max_entries {
                    bounded = true;
                    issues.push("", "entry_limit");
                    break;
                }

                let path = entry.path();
                let Ok(relative) = path.strip_prefix(&root) else {
                    issues.push_at(&path, &root, "outside_root");
                    continue;
                };
                let relative = relative.to_string_lossy().to_string();
                let Ok(file_type) = entry.file_type() else {
                    issues.push(&relative, "read_failed");
                    continue;
                };
                if file_type.is_symlink() {
                    // Never follow links: they can escape the root or form loops.
                    continue;
                }
                if file_type.is_dir() {
                    if depth >= self.max_depth {
                        issues.push(&relative, "depth_limit");
                        continue;
                    }
                    if !is_hidden_name(&entry.file_name()) {
                        queue.push_back((path, depth + 1));
                    }
                    continue;
                }
                if !file_type.is_file() || is_hidden_name(&entry.file_name()) || !is_media(&path) {
                    continue;
                }
                if !entry_has_utf8_name(&path) {
                    issues.push(&relative, "invalid_name");
                    continue;
                }

                match self.import(source_id, &root, &relative, &scan_id, token) {
                    Ok(()) => {
                        imported += 1;
                        on_update(&ScanUpdate {
                            visited,
                            imported,
                            errors: issues.snapshot(),
                            done: false,
                            cancelled: false,
                        });
                    }
                    Err(error) if error.code() == "still_downloading" => {
                        issues.push(&relative, "still_downloading");
                    }
                    Err(error) if error.code() == "unsafe_path" => {
                        issues.push(&relative, "unsafe_path");
                    }
                    Err(error) if error.code() == "cancelled" => break,
                    Err(CoreError::Io(error))
                        if error.kind() == std::io::ErrorKind::PermissionDenied =>
                    {
                        issues.push(&relative, "permission_lost");
                    }
                    Err(CoreError::Io(_)) => issues.push(&relative, "read_failed"),
                    Err(error) => return Err(error),
                }
            }
            if bounded {
                break;
            }
        }

        let cancelled = token.is_cancelled();
        let complete = !bounded && issues.is_empty();
        if !cancelled {
            self.library
                .finish_scan(source_id, &scan_id, complete, Some(&source.root_path))?;
        }
        let update = ScanUpdate {
            visited,
            imported,
            errors: issues.snapshot(),
            done: true,
            cancelled,
        };
        on_update(&update);
        Ok(ScanSummary {
            visited,
            imported,
            issues: issues.into_vec(),
            cancelled,
            complete,
        })
    }

    fn import(
        &self,
        source_id: &str,
        root: &Path,
        relative: &str,
        scan_id: &str,
        token: &ScanToken,
    ) -> Result<()> {
        let path = root.join(relative);
        let metadata = std::fs::metadata(&path)?;
        if metadata.len() == 0 || self.is_settling(metadata.modified().ok()) {
            return Err(CoreError::library(
                "still_downloading",
                "The file is still being written.",
            ));
        }
        if has_download_sidecar(&path) {
            return Err(CoreError::library(
                "still_downloading",
                "The file is still being downloaded.",
            ));
        }
        if token.is_cancelled() {
            return Err(CoreError::library("cancelled", "The scan was cancelled."));
        }

        let size = metadata.len();
        let fingerprint = media_fingerprint(&path, size)?;
        // Re-check after hashing: a growing file must not be recorded with stale facts.
        let after = std::fs::metadata(&path)?;
        if after.len() != size || after.modified().ok() != metadata.modified().ok() {
            return Err(CoreError::library(
                "still_downloading",
                "The file changed while it was being read.",
            ));
        }
        let modified_ms = after
            .modified()
            .ok()
            .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|elapsed| elapsed.as_millis() as i64)
            .unwrap_or_default();

        let mut folder = Path::new(relative)
            .parent()
            .map(|parent| parent.to_string_lossy().to_string())
            .unwrap_or_default();
        if let Some(parent) = Path::new(&folder).file_name()
            && is_season_folder(&parent.to_string_lossy())
        {
            folder = Path::new(&folder)
                .parent()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_default();
        }
        let parsed = parse_filename(relative, Some(&folder));
        self.library.ingest_file(Ingest {
            source_id,
            relative_path: relative,
            fingerprint: &fingerprint,
            size: size as i64,
            modified_ms,
            scan_id,
            parsed: Some(&parsed),
            expected_root: Some(&root.to_string_lossy()),
        })?;
        Ok(())
    }

    fn is_settling(&self, modified: Option<SystemTime>) -> bool {
        let Some(modified) = modified else {
            return false;
        };
        match SystemTime::now().duration_since(modified) {
            Ok(age) => age < self.stable_age,
            Err(_) => false,
        }
    }
}

struct ScanGuard<'a> {
    library: &'a Library,
    source_id: String,
}

impl Drop for ScanGuard<'_> {
    fn drop(&mut self) {
        self.library.release_scan(&self.source_id);
    }
}

/// Whether a directory entry is a dot-file or dot-folder.
///
/// On a volume that cannot hold resource forks — an exFAT USB drive is the
/// common case for a media library — macOS writes an AppleDouble companion
/// (`._Name.mkv`) beside every file. Those companions keep the media extension,
/// so treating them as media invents titles like `._12 Angry Men - 1957`, counts
/// every file twice and offers playback of 4 KiB of metadata. They are skipped
/// silently: they are not a scan problem the viewer can act on.
fn is_hidden_name(name: &std::ffi::OsStr) -> bool {
    name.to_string_lossy().starts_with('.')
}

fn is_media(path: &Path) -> bool {
    path.extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .is_some_and(|extension| MEDIA_EXTENSIONS.contains(&extension.as_str()))
}

fn entry_has_utf8_name(path: &Path) -> bool {
    path.file_name()
        .map(|name| name.to_str().is_some())
        .unwrap_or(false)
}

fn is_season_folder(name: &str) -> bool {
    let lower = name.to_lowercase();
    let Some(number) = lower.strip_prefix("season") else {
        return false;
    };
    let number = number.trim_start_matches([' ', '.', '_', '-']);
    !number.is_empty() && number.chars().all(|character| character.is_ascii_digit())
}

/// Which unavailability a root that will not resolve actually is.
///
/// The three cases have different recovery actions, so they are decided here rather
/// than collapsed in a view: access was withdrawn needs a re-grant, a folder that is
/// gone needs a relink, and a volume that is not attached needs plugging back in.
/// Reporting a deleted folder as "drive not attached" tells the owner to do
/// something that cannot work.
///
/// The probe is the stored root's **parent**. A parent that still exists means the
/// volume holding the folder is mounted and the folder itself has gone; a parent
/// that has also gone means the volume is not there. Nothing walks further up,
/// because an unmounted mount point's own ancestors always still exist (`/Volumes`,
/// `C:\`) and would answer "mounted" for every missing drive.
fn unavailable_root(stored_root: &Path, kind: std::io::ErrorKind) -> (SourceState, &'static str) {
    if kind == std::io::ErrorKind::PermissionDenied {
        return (SourceState::PermissionLost, "permission_lost");
    }
    match stored_root.parent() {
        Some(parent) if !parent.as_os_str().is_empty() && parent.exists() => {
            (SourceState::Missing, "source_missing")
        }
        // No parent at all is a file system root, which cannot be missing on its own.
        _ => (SourceState::Disconnected, "source_disconnected"),
    }
}

fn has_download_sidecar(path: &Path) -> bool {
    let name = path.to_string_lossy();
    DOWNLOAD_SUFFIXES
        .iter()
        .any(|suffix| Path::new(&format!("{name}{suffix}")).exists())
}

/// Bounded issue collection with an explicit truncation marker.
struct Issues {
    entries: Vec<ScanIssue>,
    max: usize,
}

impl Issues {
    fn new(max: usize) -> Self {
        Self {
            entries: Vec::new(),
            max,
        }
    }

    fn push(&mut self, relative_path: &str, code: &'static str) {
        if self.entries.len() < self.max {
            self.entries.push(ScanIssue {
                relative_path: relative_path.to_string(),
                code,
            });
        } else if self.entries.len() == self.max {
            self.entries.push(ScanIssue {
                relative_path: String::new(),
                code: "additional_errors_omitted",
            });
        }
    }

    fn push_at(&mut self, path: &Path, root: &Path, code: &'static str) {
        let relative = path
            .strip_prefix(root)
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();
        self.push(&relative, code);
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn snapshot(&self) -> Vec<ScanIssue> {
        self.entries.clone()
    }

    fn into_vec(self) -> Vec<ScanIssue> {
        self.entries
    }
}
