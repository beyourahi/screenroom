//! Local recommendations: suggestions the library can stand behind.
//!
//! Every suggestion names its reason, and the reason is something the library
//! actually knows — the next episode after the one in progress, a title on the
//! viewer's own watchlist, another work in a genre they have already started.
//! There is no model, no network call and no invented "because you liked"
//! elsewhere: a row either points at media on this machine or it is not offered.

use std::collections::HashSet;

use super::{Library, LibraryQuery, Sort};
use crate::domain::{LibraryItem, MediaKind, Recommendation};
use crate::error::Result;

/// Most suggestions one call returns.
pub const MAX_RECOMMENDATIONS: i64 = 60;

/// How many in-progress works are inspected for their next episode.
const IN_PROGRESS_SAMPLE: i64 = 10;

impl Library {
    /// Suggestions for one profile, best first, deduplicated by work.
    ///
    /// One work appears at most once: a series that has eight unwatched episodes
    /// must not fill the shelf with eight rows. The order is the order of
    /// usefulness — the next episode of something already started, then the
    /// viewer's own watchlist, then more of a genre they are watching, then what
    /// was added most recently.
    pub fn recommendations(&self, profile_id: &str, limit: i64) -> Result<Vec<Recommendation>> {
        let limit = limit.clamp(1, MAX_RECOMMENDATIONS) as usize;
        let mut out: Vec<Recommendation> = Vec::new();
        let mut seen_works: HashSet<String> = HashSet::new();
        let mut seen_files: HashSet<String> = HashSet::new();

        let in_progress = self.continue_watching(profile_id, IN_PROGRESS_SAMPLE)?;

        // 1. The next episode of a series already being watched. This is the one
        // suggestion that needs no guessing: the library knows the order. An
        // episode that was just finished counts as "already being watched" — that
        // is exactly when the next one is wanted — so the recently completed list
        // is consulted too, after the ones still in progress.
        let mut starters = in_progress.clone();
        starters.extend(self.items(&LibraryQuery {
            profile_id: Some(profile_id.to_string()),
            watched: Some(true),
            available_only: true,
            sort: Sort::Progress,
            limit: 10,
            ..LibraryQuery::default()
        })?);
        for item in starters
            .iter()
            .filter(|item| item.kind == MediaKind::Series)
        {
            let Some(next) = self.next_episode(&item.id, Some(profile_id))? else {
                continue;
            };
            push(
                &mut out,
                &mut seen_works,
                &mut seen_files,
                next,
                format!("Next episode of {}", item.title),
            );
        }

        // 2. What the viewer themselves put aside for later.
        self.offer(
            &mut out,
            &mut seen_works,
            &mut seen_files,
            LibraryQuery {
                profile_id: Some(profile_id.to_string()),
                watchlist_only: true,
                watched: Some(false),
                available_only: true,
                sort: Sort::Added,
                limit: 20,
                ..LibraryQuery::default()
            },
            "On your watchlist",
        )?;

        // 3. More of a genre already in progress. Only genres the library holds
        // are considered, and only two of them, so one dominant genre cannot
        // own the whole shelf.
        let mut genres: Vec<String> = Vec::new();
        for item in &in_progress {
            for genre in &item.genres {
                if !genres.iter().any(|known| known == genre) {
                    genres.push(genre.clone());
                }
            }
        }
        for genre in genres.iter().take(2) {
            self.offer(
                &mut out,
                &mut seen_works,
                &mut seen_files,
                LibraryQuery {
                    profile_id: Some(profile_id.to_string()),
                    genre: Some(genre.clone()),
                    watched: Some(false),
                    available_only: true,
                    sort: Sort::Title,
                    limit: 8,
                    ..LibraryQuery::default()
                },
                &format!("More {genre}"),
            )?;
        }

        // 4. Recently added, which is the honest fallback for a new library.
        self.offer(
            &mut out,
            &mut seen_works,
            &mut seen_files,
            LibraryQuery {
                profile_id: Some(profile_id.to_string()),
                watched: Some(false),
                available_only: true,
                sort: Sort::Added,
                limit: 12,
                ..LibraryQuery::default()
            },
            "Recently added",
        )?;

        out.truncate(limit);
        Ok(out)
    }

    /// Runs one query and appends results that name a work not yet offered.
    #[allow(clippy::too_many_arguments)]
    fn offer(
        &self,
        out: &mut Vec<Recommendation>,
        seen_works: &mut HashSet<String>,
        seen_files: &mut HashSet<String>,
        query: LibraryQuery,
        reason: &str,
    ) -> Result<()> {
        for item in self.items(&query)? {
            push(out, seen_works, seen_files, item, reason.to_string());
        }
        Ok(())
    }
}

/// Appends one suggestion unless its work or file was already offered.
fn push(
    out: &mut Vec<Recommendation>,
    seen_works: &mut HashSet<String>,
    seen_files: &mut HashSet<String>,
    item: LibraryItem,
    reason: String,
) {
    if !seen_works.insert(item.work_id.clone()) {
        return;
    }
    if !seen_files.insert(item.id.clone()) {
        return;
    }
    out.push(Recommendation { item, reason });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Ingest, parse_filename};

    fn library() -> (tempfile::TempDir, Library, String) {
        let directory = tempfile::tempdir().expect("tempdir");
        let library = Library::open_in_memory().expect("library");
        library
            .add_source(directory.path(), None)
            .expect("source source");
        let profile = library.profiles().expect("profiles")[0].id.clone();
        (directory, library, profile)
    }

    fn ingest(library: &Library, name: &str) -> String {
        let source = &library.sources().expect("sources")[0];
        let parsed = parse_filename(name, None);
        library
            .ingest_file(Ingest {
                source_id: &source.id,
                relative_path: name,
                fingerprint: name,
                size: 10,
                modified_ms: 1,
                scan_id: "scan-1",
                parsed: Some(&parsed),
                expected_root: Some(&source.root_path),
            })
            .expect("ingest")
    }

    /// Writes a genre onto a work, standing in for metadata the library would
    /// otherwise have no way to know about a synthetic file.
    fn set_genres(library: &Library, work_id: &str, genres: &[&str]) {
        let json = format!(
            "[{}]",
            genres
                .iter()
                .map(|genre| format!("\"{genre}\""))
                .collect::<Vec<_>>()
                .join(",")
        );
        library
            .connection()
            .execute(
                "UPDATE works SET genres = ?1 WHERE id = ?2",
                rusqlite::params![json, work_id],
            )
            .expect("genres");
    }

    #[test]
    fn the_next_episode_beats_everything_else_and_comes_with_a_reason() {
        let (directory, library, profile) = library();
        for name in ["Show.S01E01.mkv", "Show.S01E02.mkv", "Film.2020.mkv"] {
            std::fs::write(directory.path().join(name), b"video").expect("write");
            ingest(&library, name);
        }
        let first = library
            .items(&LibraryQuery {
                profile_id: Some(profile.clone()),
                text: "Show".to_string(),
                sort: Sort::Title,
                limit: 1,
                ..LibraryQuery::default()
            })
            .expect("items")[0]
            .clone();

        // Watched through the ordinary session path, so the suggestion is based
        // on the same state the UI shows.
        let session = library.start_session(&profile, &first.id).expect("session");
        library
            .checkpoint(crate::Checkpoint {
                session: &session,
                position_ms: 1_000,
                duration_ms: 2_000,
                sequence: 1,
                seek_generation: 0,
                ended: true,
            })
            .expect("checkpoint");

        let suggestions = library.recommendations(&profile, 10).expect("suggestions");
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].reason, "Next episode of Show");
        assert_eq!(suggestions[0].item.episode, Some(2));

        // No work is offered twice, whatever the reason.
        let works: HashSet<&str> = suggestions
            .iter()
            .map(|suggestion| suggestion.item.work_id.as_str())
            .collect();
        assert_eq!(works.len(), suggestions.len());
    }

    #[test]
    fn a_genre_in_progress_suggests_another_work_in_it() {
        let (directory, library, profile) = library();
        for name in [
            "Drama.One.2020.mkv",
            "Drama.Two.2021.mkv",
            "Comedy.2022.mkv",
        ] {
            std::fs::write(directory.path().join(name), b"video").expect("write");
            ingest(&library, name);
        }
        let one = library
            .item(
                &library
                    .items(&LibraryQuery {
                        text: "Drama One".to_string(),
                        limit: 1,
                        ..LibraryQuery::default()
                    })
                    .expect("items")[0]
                    .id,
                None,
            )
            .expect("read")
            .expect("present");
        set_genres(&library, &one.work_id, &["Drama"]);
        let two = library
            .items(&LibraryQuery {
                text: "Drama Two".to_string(),
                limit: 1,
                ..LibraryQuery::default()
            })
            .expect("items")[0]
            .work_id
            .clone();
        set_genres(&library, &two, &["Drama"]);

        let session = library.start_session(&profile, &one.id).expect("session");
        library
            .checkpoint(crate::Checkpoint {
                session: &session,
                position_ms: 500,
                duration_ms: 2_000,
                sequence: 1,
                seek_generation: 0,
                ended: false,
            })
            .expect("checkpoint");

        let suggestions = library.recommendations(&profile, 10).expect("suggestions");
        assert!(
            suggestions
                .iter()
                .any(|suggestion| suggestion.reason == "More Drama"),
            "{suggestions:?}"
        );
    }

    #[test]
    fn a_watched_or_missing_file_is_never_suggested() {
        let (directory, library, profile) = library();
        std::fs::write(directory.path().join("Seen.2020.mkv"), b"video").expect("write");
        let seen = ingest(&library, "Seen.2020.mkv");
        let item = library.item(&seen, None).expect("read").expect("present");
        library
            .mark_watched(&profile, &item.edition_id, true)
            .expect("watched");

        let suggestions = library.recommendations(&profile, 10).expect("suggestions");
        assert!(
            !suggestions
                .iter()
                .any(|suggestion| suggestion.item.id == seen),
            "{suggestions:?}"
        );
    }
}
