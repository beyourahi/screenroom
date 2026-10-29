//! Media facts read from the files themselves, before anyone presses play.
//!
//! A scan reads names and sizes, never containers, so every edition starts with a
//! duration of `0`. Until something fills it in, the catalogue cannot show a
//! runtime or a progress bar for an unwatched film, and a segment cannot be marked.
//! A client that can probe containers (the desktop runs ffprobe) reports the
//! duration here; the player's own report on first play still wins, because it is
//! only written where the stored value is still unknown.

use std::path::{Component, Path, PathBuf};

use rusqlite::params;

use super::Library;
use super::playback::MAX_DURATION_MS;
use crate::error::Result;

/// One file whose container has not been probed yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnprobedFile {
    /// File to probe.
    pub file_id: String,
    /// Absolute location under its granted root.
    pub path: PathBuf,
}

/// `root` joined with a stored relative path, or `None` when that path could
/// leave the root (`..`, an absolute path, a drive prefix) or is empty.
///
/// A scan never stores such a path, but a restored or hand-edited library could,
/// and these paths go straight to ffprobe and ffmpeg.
pub(crate) fn under_root(root: &str, relative: &str) -> Option<PathBuf> {
    let relative = Path::new(relative);
    let mut parts = relative.components().peekable();
    parts.peek()?;
    parts
        .all(|part| matches!(part, Component::Normal(_)))
        .then(|| PathBuf::from(root).join(relative))
}

impl Library {
    /// Available files whose edition has no known duration, in a stable order.
    ///
    /// Keyset-paged on the file id: `after` is the last id of the previous page,
    /// so a file whose probe fails is passed over instead of returning on every
    /// page and stalling the walk.
    pub fn unprobed_files(&self, after: Option<&str>, limit: i64) -> Result<Vec<UnprobedFile>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT f.id, s.root_path, f.relative_path
             FROM files f
             JOIN editions e ON e.id = f.edition_id
             JOIN sources s ON s.id = f.source_id
             WHERE f.available = 1 AND s.state = 'available' AND e.duration_ms = 0 AND f.id > ?1
             ORDER BY f.id
             LIMIT ?2",
        )?;
        let rows =
            statement.query_map(params![after.unwrap_or(""), limit.clamp(1, 500)], |row| {
                let root: String = row.get(1)?;
                let relative: String = row.get(2)?;
                let file_id: String = row.get(0)?;
                Ok(under_root(&root, &relative).map(|path| UnprobedFile { file_id, path }))
            })?;
        // shortcut: a page made only of escaping paths reads as the end of the
        // walk; only a tampered library can hold one, and the next launch resumes.
        Ok(rows
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect())
    }

    /// Records a probed duration for the file's edition, if it is still unknown.
    ///
    /// Returns whether anything was written. An impossible value is refused rather
    /// than clamped, and a duration the player already reported is never replaced.
    pub fn record_duration(&self, file_id: &str, duration_ms: i64) -> Result<bool> {
        if duration_ms <= 0 || duration_ms > MAX_DURATION_MS {
            return Ok(false);
        }
        let changed = self.connection().execute(
            "UPDATE editions SET duration_ms = ?1
             WHERE id = (SELECT edition_id FROM files WHERE id = ?2) AND duration_ms = 0",
            params![duration_ms, file_id],
        )?;
        Ok(changed > 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::filename::parse_filename;
    use crate::library::{Ingest, Library};

    fn library_with(names: &[&str]) -> (tempfile::TempDir, Library) {
        let directory = tempfile::tempdir().expect("tempdir");
        let library = Library::open_in_memory().expect("library");
        let source = library.add_source(directory.path(), None).expect("source");
        for name in names {
            std::fs::write(directory.path().join(name), b"video").expect("write");
            let parsed = parse_filename(name, None);
            library
                .ingest_file(Ingest {
                    source_id: &source.id,
                    relative_path: name,
                    fingerprint: name,
                    size: 5,
                    modified_ms: 1,
                    scan_id: "scan-1",
                    parsed: Some(&parsed),
                    expected_root: Some(&source.root_path),
                })
                .expect("ingest");
        }
        (directory, library)
    }

    #[test]
    fn a_stored_path_that_could_leave_its_root_is_never_handed_out() {
        use super::under_root;
        assert!(under_root("/media", "Films/A.mkv").is_some());
        assert_eq!(under_root("/media", "../etc/passwd"), None);
        assert_eq!(under_root("/media", "Films/../../x.mkv"), None);
        assert_eq!(under_root("/media", "/etc/passwd"), None);
        assert_eq!(under_root("/media", ""), None);
    }

    #[test]
    fn a_probed_duration_fills_an_unknown_one_and_never_replaces_a_known_one() {
        let (_dir, library) = library_with(&["A.Film.2001.mkv", "B.Film.2002.mkv"]);
        let unprobed = library.unprobed_files(None, 10).unwrap();
        assert_eq!(unprobed.len(), 2);
        assert!(unprobed[0].file_id < unprobed[1].file_id);
        assert!(unprobed.iter().all(|file| file.path.is_absolute()));

        let first = &unprobed[0].file_id;
        assert!(!library.record_duration(first, 0).unwrap());
        assert!(!library.record_duration(first, -5).unwrap());
        assert!(!library.record_duration(first, i64::MAX).unwrap());
        assert!(library.record_duration(first, 5_400_000).unwrap());
        // Known now, so a second report (or a different probe) cannot overwrite it.
        assert!(!library.record_duration(first, 1_000).unwrap());

        let left = library.unprobed_files(None, 10).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].file_id, unprobed[1].file_id);
        // Keyset paging passes a file over once its id is behind the cursor.
        assert!(
            library
                .unprobed_files(Some(&unprobed[1].file_id), 10)
                .unwrap()
                .is_empty()
        );
    }
}
