//! Retained artwork bytes.
//!
//! The rule that decides this module's shape is the offline one: a library must
//! draw without a network, so what is kept is the image itself, never a remote
//! URL. Nothing here fetches or generates an image; a client extracts a frame
//! from media it already has and hands the encoded bytes over, and this module
//! decides whether they may be stored and gives them back later.
//!
//! Artwork belongs to a **work**, not a file: every episode of a series shares
//! one poster. The file the frame came from is remembered so a re-extraction can
//! be traced, but it is not part of the identity.

use std::path::PathBuf;

use rusqlite::OptionalExtension;

use super::{Library, now_ms};
use crate::error::{CoreError, Result};

/// Largest image this library will store.
///
/// A poster is a thumbnail, not a gallery print: 768 KiB is far more than a
/// 400-pixel-wide JPEG needs, and the ceiling is what stops a mistaken caller
/// from putting a whole frame or a video file into the database.
pub const MAX_ARTWORK_BYTES: usize = 768 * 1024;

/// Largest accepted dimension, in pixels.
pub const MAX_ARTWORK_SIDE: i64 = 4096;

/// A poster as it is stored: encoded bytes plus enough to draw them honestly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artwork {
    /// Work the image belongs to.
    pub work_id: String,
    /// Media type of the bytes, currently always `image/jpeg`.
    pub mime: String,
    /// Pixel width, as extracted.
    pub width: i64,
    /// Pixel height, as extracted.
    pub height: i64,
    /// Position in the source file the frame was taken from, in milliseconds.
    pub sampled_at_ms: i64,
    /// The encoded image.
    pub bytes: Vec<u8>,
}

/// A work whose artwork is worth extracting, with the file to read it from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtworkCandidate {
    /// Work that has no artwork yet.
    pub work_id: String,
    /// The file to sample a frame from.
    pub file_id: String,
    /// Title, for progress reporting only.
    pub title: String,
    /// Known duration of the edition, or `0` when the player has not reported it.
    pub duration_ms: i64,
    /// Absolute path of the file to sample.
    pub path: PathBuf,
}

impl Library {
    /// Stores artwork for a work, replacing any earlier image for it.
    ///
    /// Validation is deliberately strict and happens before the write: the bytes
    /// must be a JPEG, within [`MAX_ARTWORK_BYTES`], and describe a plausible
    /// image. A rejected image stores nothing at all, so a failed extraction can
    /// never leave a half-written poster that the UI would then draw.
    pub fn store_artwork(
        &self,
        work_id: &str,
        file_id: Option<&str>,
        bytes: &[u8],
        width: i64,
        height: i64,
        sampled_at_ms: i64,
    ) -> Result<()> {
        if bytes.is_empty() || bytes.len() > MAX_ARTWORK_BYTES {
            return Err(CoreError::library(
                "invalid_artwork",
                "The extracted image was empty or too large to keep.",
            ));
        }
        // JPEG only: the magic number is checked rather than trusted from the
        // caller, so a truncated or mislabelled file is refused here instead of
        // failing later in the image decoder, where the reason would be invisible.
        if !bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Err(CoreError::library(
                "invalid_artwork",
                "The extracted image was not a JPEG.",
            ));
        }
        if !(1..=MAX_ARTWORK_SIDE).contains(&width) || !(1..=MAX_ARTWORK_SIDE).contains(&height) {
            return Err(CoreError::library(
                "invalid_artwork",
                "The extracted image had an unusable size.",
            ));
        }
        if sampled_at_ms < 0 {
            return Err(CoreError::library(
                "invalid_artwork",
                "The sampled position cannot be negative.",
            ));
        }

        let connection = self.connection();
        let exists: Option<String> = connection
            .query_row("SELECT id FROM works WHERE id = ?1", [work_id], |row| {
                row.get(0)
            })
            .optional()?;
        if exists.is_none() {
            return Err(CoreError::library(
                "work_missing",
                "That work is no longer in the library.",
            ));
        }

        connection.execute(
            "INSERT INTO artwork(work_id, file_id, mime, width, height, sampled_at_ms, created_ms, bytes)
             VALUES(?1, ?2, 'image/jpeg', ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(work_id) DO UPDATE SET
               file_id = excluded.file_id, mime = excluded.mime, width = excluded.width,
               height = excluded.height, sampled_at_ms = excluded.sampled_at_ms,
               created_ms = excluded.created_ms, bytes = excluded.bytes",
            rusqlite::params![
                work_id,
                file_id,
                width,
                height,
                sampled_at_ms,
                now_ms(),
                bytes
            ],
        )?;
        Ok(())
    }

    /// Reads the artwork stored for one work.
    pub fn artwork(&self, work_id: &str) -> Result<Option<Artwork>> {
        let connection = self.connection();
        let row = connection
            .query_row(
                "SELECT work_id, mime, width, height, sampled_at_ms, bytes
                 FROM artwork WHERE work_id = ?1",
                [work_id],
                |row| {
                    Ok(Artwork {
                        work_id: row.get(0)?,
                        mime: row.get(1)?,
                        width: row.get(2)?,
                        height: row.get(3)?,
                        sampled_at_ms: row.get(4)?,
                        bytes: row.get(5)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Reads the artwork for a page of works in one query.
    ///
    /// A grid asks for every card at once; one query per card would make a page
    /// of 24 cost 24 round trips, which is the difference between a page that
    /// opens instantly and one that visibly fills in.
    pub fn artworks_for(&self, work_ids: &[String]) -> Result<Vec<Artwork>> {
        if work_ids.is_empty() {
            return Ok(Vec::new());
        }
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT work_id, mime, width, height, sampled_at_ms, bytes
             FROM artwork WHERE work_id = ?1",
        )?;
        let mut found = Vec::new();
        for work_id in work_ids {
            let row = statement
                .query_row([work_id], |row| {
                    Ok(Artwork {
                        work_id: row.get(0)?,
                        mime: row.get(1)?,
                        width: row.get(2)?,
                        height: row.get(3)?,
                        sampled_at_ms: row.get(4)?,
                        bytes: row.get(5)?,
                    })
                })
                .optional()?;
            if let Some(artwork) = row {
                found.push(artwork);
            }
        }
        Ok(found)
    }

    /// Works that have no artwork yet and a readable file to sample one from.
    ///
    /// Only available files are offered: extracting a frame from a disconnected
    /// drive would fail every time, and a queue full of guaranteed failures hides
    /// the works that could actually be drawn.
    pub fn artwork_candidates(&self, limit: i64) -> Result<Vec<ArtworkCandidate>> {
        self.candidates(None, limit.clamp(1, 200))
    }

    /// Candidates restricted to the works a screen is showing.
    ///
    /// A grid must draw what is on it before it fills in the rest of the library:
    /// a viewer looking at a page of 24 cards cares about those 24 works, not
    /// about the alphabetically first ones, so the visible set is filtered in the
    /// query rather than by trimming a global page — which would return almost
    /// nothing whenever the library is larger than one page.
    pub fn artwork_candidates_in(&self, work_ids: &[String]) -> Result<Vec<ArtworkCandidate>> {
        if work_ids.is_empty() {
            return Ok(Vec::new());
        }
        self.candidates(Some(work_ids), work_ids.len().min(200) as i64)
    }

    /// The one candidate query, optionally restricted to named works.
    fn candidates(&self, work_ids: Option<&[String]>, limit: i64) -> Result<Vec<ArtworkCandidate>> {
        let filter = match work_ids {
            Some(ids) => format!(
                " AND w.id IN ({})",
                ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
            ),
            None => String::new(),
        };
        let sql = format!(
            "SELECT w.id, f.id, w.title, e.duration_ms, s.root_path, f.relative_path
             FROM works w
             JOIN files f ON f.id = (
                 SELECT f2.id FROM files f2
                 JOIN editions e2 ON e2.id = f2.edition_id
                 WHERE e2.work_id = w.id AND f2.available = 1
                 ORDER BY COALESCE(f2.season_number, 0), COALESCE(f2.episode_number, 0),
                          f2.relative_path
                 LIMIT 1)
             JOIN editions e ON e.id = f.edition_id
             JOIN sources s ON s.id = f.source_id
             WHERE NOT EXISTS (SELECT 1 FROM artwork a WHERE a.work_id = w.id){filter}
             ORDER BY w.title COLLATE NOCASE
             LIMIT ?"
        );
        let mut values: Vec<&dyn rusqlite::ToSql> = Vec::new();
        if let Some(ids) = work_ids {
            for id in ids {
                values.push(id);
            }
        }
        let limit = limit.to_string();
        values.push(&limit);

        let connection = self.connection();
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(rusqlite::params_from_iter(values), |row| {
            let root: String = row.get(4)?;
            let relative: String = row.get(5)?;
            Ok(ArtworkCandidate {
                work_id: row.get(0)?,
                file_id: row.get(1)?,
                title: row.get(2)?,
                duration_ms: row.get(3)?,
                path: PathBuf::from(root).join(relative),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// How many works still have no artwork.
    pub fn missing_artwork_count(&self) -> Result<i64> {
        Ok(self.connection().query_row(
            "SELECT COUNT(*) FROM works w
             WHERE NOT EXISTS (SELECT 1 FROM artwork a WHERE a.work_id = w.id)",
            [],
            |row| row.get(0),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Ingest, SCHEMA_VERSION, parse_filename, schema};

    /// A minimal JPEG header, enough for the magic-number check.
    fn jpeg() -> Vec<u8> {
        vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F']
    }

    /// A library holding one ingested file, with its work and file identifiers.
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
        (directory, library, item.work_id, file_id)
    }

    #[test]
    fn a_schema_two_library_is_migrated_without_losing_rows() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("library.db");
        {
            // Everything a schema 2 library had, and nothing else: the artwork
            // table did not exist and its absence is what the migration adds.
            let connection = rusqlite::Connection::open(&path).expect("open");
            for statement in &schema::CREATE[..schema::CREATE.len() - 1] {
                connection.execute(statement, []).expect("schema");
            }
            connection
                .execute("INSERT INTO profiles(id, name) VALUES('p', 'Me')", [])
                .expect("profile");
            connection
                .pragma_update(None, "user_version", 2)
                .expect("version");
        }

        let library = Library::open(&path).expect("migrated");
        let version: i64 = library
            .connection()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("version");
        assert_eq!(version, SCHEMA_VERSION);
        assert_eq!(library.profiles().expect("profiles").len(), 1);
        // The new table is usable immediately.
        assert!(library.artwork("nothing").expect("read").is_none());
    }

    #[test]
    fn only_a_jpeg_of_a_sane_size_is_stored() {
        let (_directory, library, work, file_id) = one_file("Some.Film.2020.mkv");

        let refused =
            library.store_artwork(&work, Some(&file_id), b"not an image", 400, 600, 1_000);
        assert_eq!(refused.unwrap_err().code(), "invalid_artwork");
        assert!(library.artwork(&work).expect("read").is_none());

        let mut oversized = jpeg();
        oversized.resize(MAX_ARTWORK_BYTES + 1, 0);
        assert_eq!(
            library
                .store_artwork(&work, Some(&file_id), &oversized, 400, 600, 1_000)
                .unwrap_err()
                .code(),
            "invalid_artwork"
        );

        assert_eq!(
            library
                .store_artwork(&work, Some(&file_id), &jpeg(), 0, 600, 1_000)
                .unwrap_err()
                .code(),
            "invalid_artwork"
        );

        library
            .store_artwork(&work, Some(&file_id), &jpeg(), 400, 600, 1_000)
            .expect("store");
        let stored = library.artwork(&work).expect("read").expect("present");
        assert_eq!(stored.bytes, jpeg());
        assert_eq!((stored.width, stored.height), (400, 600));
        assert_eq!(stored.mime, "image/jpeg");

        // Replacing is an overwrite, not a second row: one poster per work.
        library
            .store_artwork(&work, Some(&file_id), &jpeg(), 200, 300, 4_000)
            .expect("replace");
        let replaced = library.artwork(&work).expect("read").expect("present");
        assert_eq!((replaced.width, replaced.height), (200, 300));
        assert_eq!(replaced.sampled_at_ms, 4_000);
    }

    #[test]
    fn a_page_of_artwork_comes_back_in_one_query_and_only_for_stored_works() {
        let (_directory, library, work, file_id) = one_file("Some.Film.2020.mkv");
        library
            .store_artwork(&work, Some(&file_id), &jpeg(), 400, 600, 1_000)
            .expect("store");

        let found = library
            .artworks_for(&[work.clone(), "absent".to_string()])
            .expect("read");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].work_id, work);
        assert!(library.artworks_for(&[]).expect("empty").is_empty());
    }

    #[test]
    fn candidates_name_one_readable_file_per_work_without_artwork() {
        let (_directory, library, work, _file_id) = one_file("Some.Film.2020.mkv");
        let candidates = library.artwork_candidates(10).expect("candidates");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].work_id, work);
        assert!(candidates[0].path.exists(), "a candidate must be readable");
        assert_eq!(library.missing_artwork_count().expect("count"), 1);

        // The visible-set query filters in SQL, so a work that is not on the
        // screen is never offered and the one that is, is.
        let wrong = library
            .artwork_candidates_in(&["another-work".to_string()])
            .expect("wrong");
        assert!(wrong.is_empty());
        let right = library
            .artwork_candidates_in(std::slice::from_ref(&work))
            .expect("right");
        assert_eq!(right.len(), 1);

        // A work that has artwork is not offered again.
        library
            .store_artwork(&work, None, &jpeg(), 400, 600, 1_000)
            .expect("store");
        assert!(library.artwork_candidates(10).expect("again").is_empty());
        assert_eq!(library.missing_artwork_count().expect("count"), 0);
    }

    #[test]
    fn an_unknown_work_is_refused_so_artwork_cannot_float_free() {
        let library = Library::open_in_memory().expect("library");
        assert_eq!(
            library
                .store_artwork("nope", None, &jpeg(), 10, 10, 0)
                .unwrap_err()
                .code(),
            "work_missing"
        );
    }
}
