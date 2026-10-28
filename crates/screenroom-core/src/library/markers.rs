//! Verifiable skip segments: intros and credits.
//!
//! Nothing here guesses. A marker exists only when something that could know
//! said where the segment starts and ends, and it records which — `user` for a
//! position the viewer pinned while watching, `manual` for an imported or
//! corrected one. A marker that would not fit inside the edition it describes is
//! refused rather than clamped, because a skipped stretch of the wrong film is
//! worse than no button at all.

use rusqlite::OptionalExtension;

use super::{Library, new_id};
use crate::domain::SegmentMarker;
use crate::error::{CoreError, Result};

/// Start of the title sequence.
pub const INTRO: &str = "intro";
/// End credits.
pub const CREDITS: &str = "credits";

/// The marker kinds this library accepts.
pub const KINDS: [&str; 2] = [INTRO, CREDITS];

impl Library {
    /// Records where an intro or credits segment is, replacing any marker of the
    /// same kind on that edition.
    ///
    /// One marker per kind per edition is the rule the player's button relies on:
    /// two overlapping intro markers would make "Skip intro" ambiguous, and the
    /// second viewing would have no way to choose.
    pub fn set_marker(
        &self,
        edition_id: &str,
        kind: &str,
        start_ms: i64,
        end_ms: i64,
        duration_ms: i64,
        origin: &str,
    ) -> Result<SegmentMarker> {
        let kind = kind.trim().to_ascii_lowercase();
        if !KINDS.contains(&kind.as_str()) {
            return Err(CoreError::library(
                "invalid_marker",
                "Only intro and credits markers are supported.",
            ));
        }
        if start_ms < 0 || end_ms <= start_ms {
            return Err(CoreError::library(
                "invalid_marker",
                "The segment must end after it starts.",
            ));
        }
        if duration_ms < end_ms {
            return Err(CoreError::library(
                "invalid_marker",
                "The segment has to fit inside the film.",
            ));
        }
        if origin.trim().is_empty() {
            return Err(CoreError::library(
                "invalid_marker",
                "A marker has to say where it came from.",
            ));
        }

        let mut connection = self.connection();
        let tx = connection.transaction()?;
        let edition: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM editions WHERE id = ?1",
                [edition_id],
                |row| row.get(0),
            )
            .optional()?;
        if edition.is_none() {
            return Err(CoreError::library(
                "edition_missing",
                "That edition is no longer in the library.",
            ));
        }
        tx.execute(
            "DELETE FROM markers WHERE edition_id = ?1 AND kind = ?2",
            [edition_id, kind.as_str()],
        )?;
        let marker = SegmentMarker {
            id: new_id(),
            edition_id: edition_id.to_string(),
            kind: kind.clone(),
            start_ms,
            end_ms,
            duration_ms,
        };
        tx.execute(
            "INSERT INTO markers(id, edition_id, kind, start_ms, end_ms, duration_ms, origin)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                marker.id,
                marker.edition_id,
                marker.kind,
                marker.start_ms,
                marker.end_ms,
                marker.duration_ms,
                origin.trim()
            ],
        )?;
        tx.commit()?;
        Ok(marker)
    }

    /// Every marker stored for an edition, earliest first.
    pub fn markers(&self, edition_id: &str) -> Result<Vec<SegmentMarker>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, edition_id, kind, start_ms, end_ms, duration_ms
             FROM markers WHERE edition_id = ?1 ORDER BY start_ms",
        )?;
        let rows = statement.query_map([edition_id], |row| {
            Ok(SegmentMarker {
                id: row.get(0)?,
                edition_id: row.get(1)?,
                kind: row.get(2)?,
                start_ms: row.get(3)?,
                end_ms: row.get(4)?,
                duration_ms: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Removes one marker by identifier.
    pub fn remove_marker(&self, id: &str) -> Result<()> {
        let changed = self
            .connection()
            .execute("DELETE FROM markers WHERE id = ?1", [id])?;
        if changed == 0 {
            return Err(CoreError::library(
                "marker_missing",
                "That marker is no longer stored.",
            ));
        }
        Ok(())
    }

    /// Records a segment the viewer pinned while watching.
    ///
    /// The duration comes from the library rather than the caller, so a marker
    /// can never be validated against a number the player made up, and an edition
    /// whose length is still unknown is refused with the reason it is refused
    /// instead of storing a marker that the next viewing cannot check.
    pub fn mark_segment(
        &self,
        edition_id: &str,
        kind: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<SegmentMarker> {
        let duration_ms: i64 = self
            .connection()
            .query_row(
                "SELECT duration_ms FROM editions WHERE id = ?1",
                [edition_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| {
                CoreError::library(
                    "edition_missing",
                    "That edition is no longer in the library.",
                )
            })?;
        if duration_ms <= 0 {
            return Err(CoreError::library(
                "duration_unknown",
                "Play the file once so its length is known before marking a segment.",
            ));
        }
        self.set_marker(edition_id, kind, start_ms, end_ms, duration_ms, "user")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Ingest, parse_filename};

    fn one_file(name: &str) -> (tempfile::TempDir, Library, String, String) {
        let directory = tempfile::tempdir().expect("tempdir");
        let library = Library::open_in_memory().expect("library");
        let source = library.add_source(directory.path(), None).expect("source");
        std::fs::write(directory.path().join(name), b"video").expect("write");
        let parsed = parse_filename(name, None);
        let file_id = library
            .ingest_file(Ingest {
                source_id: &source.id,
                relative_path: name,
                fingerprint: "fingerprint",
                size: 5,
                modified_ms: 1,
                scan_id: "scan-1",
                parsed: Some(&parsed),
                expected_root: Some(&source.root_path),
            })
            .expect("ingest");
        let item = library
            .item(&file_id, None)
            .expect("item read")
            .expect("item present");
        (directory, library, item.edition_id, file_id)
    }

    #[test]
    fn a_marker_round_trips_and_replaces_its_own_kind() {
        let (_dir, library, edition, _file) = one_file("Some.Film.2020.mkv");
        let first = library
            .set_marker(&edition, "intro", 5_000, 60_000, 5_400_000, "user")
            .expect("intro");
        assert_eq!(first.start_ms, 5_000);

        let second = library
            .set_marker(&edition, "intro", 6_000, 62_000, 5_400_000, "user")
            .expect("replace");
        library
            .set_marker(&edition, "credits", 5_300_000, 5_400_000, 5_400_000, "user")
            .expect("credits");

        let markers = library.markers(&edition).expect("markers");
        assert_eq!(markers.len(), 2);
        assert!(markers.iter().any(|marker| marker.id == second.id));
        assert!(!markers.iter().any(|marker| marker.id == first.id));
        let kinds: Vec<&str> = markers.iter().map(|marker| marker.kind.as_str()).collect();
        assert_eq!(kinds, [INTRO, CREDITS]);
    }

    #[test]
    fn a_segment_that_cannot_fit_is_refused() {
        let (_dir, library, edition, _file) = one_file("Some.Film.2020.mkv");
        assert_eq!(
            library
                .set_marker(&edition, "intro", 60_000, 5_000, 5_400_000, "user")
                .unwrap_err()
                .code(),
            "invalid_marker"
        );
        assert_eq!(
            library
                .set_marker(&edition, "intro", 0, 60_000, 30_000, "user")
                .unwrap_err()
                .code(),
            "invalid_marker"
        );
        assert_eq!(
            library
                .set_marker(&edition, "advert", 0, 60_000, 5_400_000, "user")
                .unwrap_err()
                .code(),
            "invalid_marker"
        );
        assert!(library.markers(&edition).expect("markers").is_empty());
    }

    #[test]
    fn marking_needs_a_known_duration_and_uses_the_stored_one() {
        let (_dir, library, edition, _file) = one_file("Some.Film.2020.mkv");
        assert_eq!(
            library
                .mark_segment(&edition, INTRO, 5_000, 60_000)
                .unwrap_err()
                .code(),
            "duration_unknown"
        );
    }
}
