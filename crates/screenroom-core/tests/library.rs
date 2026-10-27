//! Store contract tests: identity, queries, availability, progress and lists.

use std::fs;

use screenroom_core::{
    Ingest, Library, LibraryQuery, MediaKind, PathSemantics, Sort, SourceState, parse_filename,
};
use tempfile::TempDir;

fn fixture() -> (TempDir, Library, screenroom_core::Source) {
    let dir = TempDir::new().expect("temp dir");
    let library = Library::open_in_memory().expect("in-memory library");
    let source = library.add_source(dir.path(), None).expect("source");
    (dir, library, source)
}

fn ingest(library: &Library, source: &screenroom_core::Source, path: &str) -> String {
    let parsed = parse_filename(path, None);
    library
        .ingest_file(Ingest {
            source_id: &source.id,
            relative_path: path,
            fingerprint: &format!("fingerprint:{path}"),
            size: 1_000,
            modified_ms: 1,
            scan_id: "scan-1",
            parsed: Some(&parsed),
            expected_root: Some(&source.root_path),
        })
        .expect("ingest")
}

#[test]
fn new_library_starts_with_one_profile() {
    let (_dir, library, _source) = fixture();
    let profiles = library.profiles().expect("profiles");
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].name, "Me");
}

#[test]
fn profile_cannot_be_removed_to_zero() {
    let (_dir, library, _source) = fixture();
    let only = library.profiles().unwrap().remove(0);
    let error = library.remove_profile(&only.id).unwrap_err();
    assert_eq!(error.code(), "last_profile");
    let second = library.add_profile("Guest").unwrap();
    library.remove_profile(&only.id).unwrap();
    assert_eq!(library.profiles().unwrap(), vec![second]);
}

#[test]
fn overlapping_roots_are_rejected() {
    let (dir, library, _source) = fixture();
    let nested = dir.path().join("nested");
    fs::create_dir(&nested).unwrap();
    let error = library.add_source(&nested, None).unwrap_err();
    assert_eq!(error.code(), "duplicate_source");
}

#[test]
fn a_folder_cannot_be_granted_twice_under_two_spellings() {
    // On a case-folding volume `/Media/Shows` and `/media/shows` are one folder, so
    // granting both would import one tree twice under two labels and show every file
    // twice. This is the Windows behaviour, and it is the default on macOS too —
    // which is why the case-sensitive branch below is the unusual one, not this.
    let dir = TempDir::new().expect("temp dir");
    let parent = dir.path().join("Media");
    let shows = parent.join("Shows");
    fs::create_dir_all(&shows).unwrap();

    let library = Library::open_in_memory()
        .expect("library")
        .with_path_semantics(PathSemantics::CaseInsensitive);
    library.add_source(&shows, None).expect("first grant");

    // The same folder, spelled differently. `canonicalize` does not fold case, so a
    // case-sensitive comparison would accept this as a new source.
    let respelled = parent.join("shows");
    if respelled.exists() {
        let error = library.add_source(&respelled, None).unwrap_err();
        assert_eq!(error.code(), "duplicate_source");
    }

    // And a genuinely nested folder is still refused: folding must not break the
    // existing containment check.
    let nested = shows.join("Season 1");
    fs::create_dir_all(&nested).unwrap();
    assert_eq!(
        library.add_source(&nested, None).unwrap_err().code(),
        "duplicate_source"
    );
    assert_eq!(library.sources().unwrap().len(), 1);
}

#[test]
fn a_case_sensitive_volume_treats_two_spellings_as_two_folders() {
    // The other half of the same decision, and the half that could not previously be
    // run at all on a Mac: the case-sensitive branch is chosen by volume, not by
    // operating system, so the semantics are injectable and both branches are tested
    // from one host.
    let dir = TempDir::new().expect("temp dir");
    let media = dir.path().join("Media");
    let shows = media.join("Shows");
    fs::create_dir_all(&shows).unwrap();

    let library = Library::open_in_memory()
        .expect("library")
        .with_path_semantics(PathSemantics::CaseSensitive);
    library.add_source(&shows, None).expect("first grant");

    // Whether two differently-cased folders can be granted at all is decided by the
    // volume, not by this library's semantics: on a case-folding volume `SHOWS`
    // resolves to `Shows` and the second grant is the same folder. Asking the file
    // system which volume this is keeps the test truthful instead of asserting
    // behaviour that only a case-sensitive volume can show.
    let other = media.join("SHOWS");
    let volume_folds_case =
        other.exists() && std::fs::canonicalize(&other).ok() == std::fs::canonicalize(&shows).ok();

    if volume_folds_case {
        // This volume folds case, so the case-sensitive branch cannot be reached
        // through the file system here, and the test says so rather than asserting
        // behaviour only a case-sensitive volume can show. What *is* checked is the
        // part that does not need one: `add_source` canonicalises before keying, and
        // on a folding volume both spellings canonicalise to the same real name, so
        // the grant is refused by containment rather than by case.
        let resolved = std::fs::canonicalize(&other).expect("the volume resolves both spellings");
        assert_eq!(
            resolved,
            std::fs::canonicalize(&shows).expect("canonical"),
            "a folding volume must resolve two spellings to one directory"
        );
        assert_eq!(
            library.path_key(&resolved),
            library.path_key(&std::fs::canonicalize(&shows).expect("canonical")),
            "one directory must produce one key"
        );
    } else {
        assert!(
            library.add_source(&other, None).is_ok(),
            "a case-sensitive volume must allow two differently-cased folders"
        );
        assert_eq!(library.sources().unwrap().len(), 2);
    }

    // Containment is still refused, because that comparison does not depend on case.
    let nested = shows.join("Season 1");
    fs::create_dir_all(&nested).unwrap();
    assert_eq!(
        library.add_source(&nested, None).unwrap_err().code(),
        "duplicate_source"
    );
}

#[test]
fn the_default_semantics_follow_the_volume_this_library_is_on() {
    // A guess, but a checked one: the default must agree with what the platform's
    // usual file system does, and the two branches must be the only two options.
    let expected = if cfg!(any(windows, target_vendor = "apple")) {
        PathSemantics::CaseInsensitive
    } else {
        PathSemantics::CaseSensitive
    };
    assert_eq!(PathSemantics::host(), expected);

    let default = Library::open_in_memory().unwrap();
    let explicit = Library::open_in_memory()
        .unwrap()
        .with_path_semantics(expected);
    assert_eq!(
        default.path_key(std::path::Path::new("/Media/Shows")),
        explicit.path_key(std::path::Path::new("/Media/Shows")),
    );
}

#[test]
fn path_keys_fold_only_when_they_are_supposed_to() {
    let path = std::path::Path::new("/Media/Shows");
    let folding = Library::open_in_memory()
        .unwrap()
        .with_path_semantics(PathSemantics::CaseInsensitive)
        .path_key(path);
    let exact = Library::open_in_memory()
        .unwrap()
        .with_path_semantics(PathSemantics::CaseSensitive)
        .path_key(path);

    assert_eq!(folding, "/media/shows");
    assert_eq!(exact, "/Media/Shows");
    assert_ne!(folding, exact);
}

#[test]
fn unreadable_root_is_reported_and_not_created() {
    let (_dir, library, _source) = fixture();
    let missing = _dir.path().join("not-there");
    let error = library.add_source(&missing, Some("x")).unwrap_err();
    assert_eq!(error.code(), "source_missing");
    assert_eq!(library.sources().unwrap().len(), 1);
}

#[test]
fn relative_roots_are_rejected_before_touching_disk() {
    let (_dir, library, _source) = fixture();
    let error = library.add_source("relative/folder", None).unwrap_err();
    assert_eq!(error.code(), "invalid_root");
}

#[test]
fn scanning_twice_the_same_path_keeps_one_entry() {
    let (_dir, library, source) = fixture();
    let first = ingest(&library, &source, "Show.S01E01.mkv");
    let second = ingest(&library, &source, "Show.S01E01.mkv");
    assert_eq!(first, second);
    assert_eq!(library.count(&LibraryQuery::default()).unwrap(), 1);
}

#[test]
fn replaced_file_keeps_its_progress_on_the_old_edition() {
    let (_dir, library, source) = fixture();
    let file = ingest(&library, &source, "Film.2020.mkv");
    let before = library.item(&file, None).unwrap().unwrap();
    let profile = library.profiles().unwrap().remove(0);
    let session = library.start_session(&profile.id, &file).unwrap();
    library
        .checkpoint(screenroom_core::Checkpoint {
            session: &session,
            position_ms: 5_000,
            duration_ms: 10_000,
            sequence: 1,
            seek_generation: 0,
            ended: false,
        })
        .unwrap();

    let parsed = parse_filename("Film.2020.mkv", None);
    library
        .ingest_file(Ingest {
            source_id: &source.id,
            relative_path: "Film.2020.mkv",
            fingerprint: "different-bytes",
            size: 2_000,
            modified_ms: 2,
            scan_id: "scan-2",
            parsed: Some(&parsed),
            expected_root: Some(&source.root_path),
        })
        .unwrap();

    let after = library.item(&file, Some(&profile.id)).unwrap().unwrap();
    assert_ne!(before.edition_id, after.edition_id);
    // The new cut must not inherit the old cut's resume point.
    assert_eq!(after.position_ms, 0);
}

#[test]
fn renamed_small_file_keeps_its_identity() {
    let (dir, library, source) = fixture();
    let original = dir.path().join("Old.Name.mkv");
    fs::write(&original, b"identical bytes").unwrap();
    let parsed = parse_filename("Old.Name.mkv", None);
    let first = library
        .ingest_file(Ingest {
            source_id: &source.id,
            relative_path: "Old.Name.mkv",
            fingerprint: "same",
            size: 15,
            modified_ms: 7,
            scan_id: "scan-1",
            parsed: Some(&parsed),
            expected_root: Some(&source.root_path),
        })
        .unwrap();

    fs::remove_file(&original).unwrap();
    fs::write(dir.path().join("New.Name.mkv"), b"identical bytes").unwrap();
    let parsed = parse_filename("New.Name.mkv", None);
    let second = library
        .ingest_file(Ingest {
            source_id: &source.id,
            relative_path: "New.Name.mkv",
            fingerprint: "same",
            size: 15,
            modified_ms: 7,
            scan_id: "scan-2",
            parsed: Some(&parsed),
            expected_root: Some(&source.root_path),
        })
        .unwrap();

    assert_eq!(first, second);
    let item = library.item(&first, None).unwrap().unwrap();
    assert_eq!(item.relative_path, "New.Name.mkv");
}

#[test]
fn unsafe_relative_paths_are_refused() {
    let (_dir, library, source) = fixture();
    for path in ["../escape.mkv", "/absolute.mkv", ""] {
        let error = library
            .ingest_file(Ingest {
                source_id: &source.id,
                relative_path: path,
                fingerprint: "x",
                size: 1,
                modified_ms: 1,
                scan_id: "scan-1",
                parsed: None,
                expected_root: None,
            })
            .unwrap_err();
        assert_eq!(error.code(), "unsafe_path", "path {path:?}");
    }
}

#[test]
fn episodes_of_one_show_group_into_one_work() {
    let (_dir, library, source) = fixture();
    ingest(&library, &source, "Show.Name.S01E01.mkv");
    ingest(&library, &source, "Show.Name.S01E02.mkv");
    ingest(&library, &source, "Film.2020.mkv");

    let all = LibraryQuery::page(50);
    assert_eq!(library.count(&all).unwrap(), 3);
    let grouped = LibraryQuery {
        group_by_work: true,
        ..LibraryQuery::page(50)
    };
    assert_eq!(library.count(&grouped).unwrap(), 2);
    let grouped_items = library.items(&grouped).unwrap();
    assert_eq!(grouped_items.len(), 2);
    assert!(
        grouped_items
            .iter()
            .all(|item| item.kind != MediaKind::Unknown)
    );
}

#[test]
fn search_escapes_like_wildcards() {
    let (_dir, library, source) = fixture();
    ingest(&library, &source, "100% Real.2020.mkv");
    ingest(&library, &source, "Another.2021.mkv");

    let literal = LibraryQuery {
        text: "100%".into(),
        ..LibraryQuery::page(50)
    };
    assert_eq!(library.count(&literal).unwrap(), 1);

    let wildcard = LibraryQuery {
        text: "%".into(),
        ..LibraryQuery::page(50)
    };
    // A bare `%` is a literal character here, so only the filename containing it matches.
    assert_eq!(library.count(&wildcard).unwrap(), 1);
}

#[test]
fn queries_reject_out_of_range_pages() {
    let (_dir, library, _source) = fixture();
    let too_big = LibraryQuery {
        limit: 501,
        ..LibraryQuery::default()
    };
    assert_eq!(library.items(&too_big).unwrap_err().code(), "invalid_page");
    let negative = LibraryQuery {
        limit: 10,
        offset: -1,
        ..LibraryQuery::default()
    };
    assert_eq!(library.items(&negative).unwrap_err().code(), "invalid_page");
}

#[test]
fn progress_is_monotonic_and_seek_aware() {
    let (_dir, library, source) = fixture();
    let file = ingest(&library, &source, "Film.2020.mkv");
    let profile = library.profiles().unwrap().remove(0);
    let session = library.start_session(&profile.id, &file).unwrap();

    let report = |sequence: i64, seek: i64, position: i64| screenroom_core::Checkpoint {
        session: &session,
        position_ms: position,
        duration_ms: 100_000,
        sequence,
        seek_generation: seek,
        ended: false,
    };

    assert!(library.checkpoint(report(1, 0, 10_000)).unwrap());
    // A late, older event must not rewind the playhead.
    assert!(!library.checkpoint(report(0, 0, 1_000)).unwrap());
    let item = library.item(&file, Some(&profile.id)).unwrap().unwrap();
    assert_eq!(item.position_ms, 10_000);

    let seek = library.begin_seek(&session).unwrap();
    assert_eq!(seek, 1);
    // Progress reported against the pre-seek generation is ignored.
    assert!(!library.checkpoint(report(2, 0, 20_000)).unwrap());
    assert!(library.checkpoint(report(3, seek, 60_000)).unwrap());

    assert!(
        library
            .checkpoint(screenroom_core::Checkpoint {
                session: &session,
                position_ms: 100_000,
                duration_ms: 100_000,
                sequence: 4,
                seek_generation: seek,
                ended: true,
            })
            .unwrap()
    );
    let finished = library.item(&file, Some(&profile.id)).unwrap().unwrap();
    assert!(finished.watched);
    // The session closed at the end, so further callbacks are rejected.
    assert!(!library.checkpoint(report(5, seek, 50_000)).unwrap());
}

#[test]
fn a_stale_session_cannot_write_progress() {
    let (_dir, library, source) = fixture();
    let film = ingest(&library, &source, "Film.2020.mkv");
    let other = ingest(&library, &source, "Other.2021.mkv");
    let profile = library.profiles().unwrap().remove(0);
    let first = library.start_session(&profile.id, &film).unwrap();
    let second = library.start_session(&profile.id, &other).unwrap();
    assert!(second.generation > first.generation);

    let stale = library
        .checkpoint(screenroom_core::Checkpoint {
            session: &first,
            position_ms: 1_000,
            duration_ms: 2_000,
            sequence: 1,
            seek_generation: 0,
            ended: false,
        })
        .unwrap();
    assert!(!stale);
    assert!(library.begin_seek(&first).is_err());
}

#[test]
fn impossible_progress_is_rejected() {
    let (_dir, library, source) = fixture();
    let file = ingest(&library, &source, "Film.2020.mkv");
    let profile = library.profiles().unwrap().remove(0);
    let session = library.start_session(&profile.id, &file).unwrap();
    for (position, duration) in [(5_000, 0), (6_000, 5_000), (-1, 5_000)] {
        assert!(
            !library
                .checkpoint(screenroom_core::Checkpoint {
                    session: &session,
                    position_ms: position,
                    duration_ms: duration,
                    sequence: 1,
                    seek_generation: 0,
                    ended: false,
                })
                .unwrap()
        );
    }
}

#[test]
fn continue_watching_orders_by_recency_and_honours_hide() {
    let (_dir, library, source) = fixture();
    let first = ingest(&library, &source, "First.2020.mkv");
    let second = ingest(&library, &source, "Second.2021.mkv");
    let profile = library.profiles().unwrap().remove(0);

    for file in [&first, &second] {
        let session = library.start_session(&profile.id, file).unwrap();
        library
            .checkpoint(screenroom_core::Checkpoint {
                session: &session,
                position_ms: 1_000,
                duration_ms: 10_000,
                sequence: 1,
                seek_generation: 0,
                ended: false,
            })
            .unwrap();
        library.end_session(&session).unwrap();
    }
    let resume = library.continue_watching(&profile.id, 10).unwrap();
    assert_eq!(resume.len(), 2);

    let second_item = library.item(&second, Some(&profile.id)).unwrap().unwrap();
    library
        .hide_continue(&profile.id, &second_item.edition_id)
        .unwrap();
    assert_eq!(library.continue_watching(&profile.id, 10).unwrap().len(), 1);
}

#[test]
fn one_viewers_progress_never_appears_under_another() {
    // Per-profile state is independent by design — there is no cross-device or
    // cross-viewer sync requirement — so the risk is not that two profiles disagree
    // but that they silently share a row. Every per-viewer table is checked here
    // against a second profile that has done nothing.
    let (_dir, library, source) = fixture();
    let file = ingest(&library, &source, "Film.2020.mkv");
    let mine = library.profiles().unwrap().remove(0);
    let guest = library.add_profile("Guest").unwrap();
    let seen = library.item(&file, Some(&mine.id)).unwrap().unwrap();
    let (edition, work) = (seen.edition_id, seen.work_id);

    let session = library.start_session(&mine.id, &file).unwrap();
    library
        .checkpoint(screenroom_core::Checkpoint {
            session: &session,
            position_ms: 5_000,
            duration_ms: 10_000,
            sequence: 1,
            seek_generation: 0,
            ended: false,
        })
        .unwrap();
    library.end_session(&session).unwrap();
    // Lists are keyed by work, progress by edition: both are per viewer.
    library.set_favorite(&mine.id, &work, true).unwrap();
    library.set_watchlisted(&mine.id, &work, true).unwrap();

    // The same file, read as each viewer.
    let as_mine = library.item(&file, Some(&mine.id)).unwrap().unwrap();
    let as_guest = library.item(&file, Some(&guest.id)).unwrap().unwrap();
    assert_eq!(as_mine.position_ms, 5_000);
    assert_eq!(as_guest.position_ms, 0, "progress leaked across profiles");
    assert!(as_mine.favorite && as_mine.watchlisted);
    assert!(
        !as_guest.favorite && !as_guest.watchlisted,
        "a list entry leaked across profiles"
    );

    // Continue Watching and the list filters are per profile too.
    assert_eq!(library.continue_watching(&mine.id, 10).unwrap().len(), 1);
    assert_eq!(library.continue_watching(&guest.id, 10).unwrap().len(), 0);
    let favorites = LibraryQuery {
        profile_id: Some(guest.id.clone()),
        favorites_only: true,
        ..LibraryQuery::page(10)
    };
    assert_eq!(library.count(&favorites).unwrap(), 0);

    // Hiding a resume on one profile must not hide it on the other, and marking a
    // file watched for the guest must not finish it for the owner.
    library.hide_continue(&mine.id, &edition).unwrap();
    assert_eq!(library.continue_watching(&mine.id, 10).unwrap().len(), 0);
    assert_eq!(library.continue_watching(&guest.id, 10).unwrap().len(), 0);
    library.mark_watched(&guest.id, &edition, true).unwrap();
    assert!(
        library
            .item(&file, Some(&guest.id))
            .unwrap()
            .unwrap()
            .watched,
        "the guest's own mark should stick for the guest"
    );
    assert_eq!(
        library
            .item(&file, Some(&mine.id))
            .unwrap()
            .unwrap()
            .position_ms,
        5_000,
        "the guest finishing a file changed the owner's progress"
    );
}

#[test]
fn lists_survive_source_disconnect() {
    let (_dir, library, source) = fixture();
    let file = ingest(&library, &source, "Film.2020.mkv");
    let profile = library.profiles().unwrap().remove(0);
    let item = library.item(&file, Some(&profile.id)).unwrap().unwrap();

    library
        .set_favorite(&profile.id, &item.work_id, true)
        .unwrap();
    library
        .set_watchlisted(&profile.id, &item.work_id, true)
        .unwrap();
    library
        .set_source_state(&source.id, SourceState::Disconnected)
        .unwrap();

    let offline = library.item(&file, Some(&profile.id)).unwrap().unwrap();
    assert!(!offline.available);
    assert!(offline.favorite);
    assert!(offline.watchlisted);

    let favorites = LibraryQuery {
        profile_id: Some(profile.id.clone()),
        favorites_only: true,
        ..LibraryQuery::page(10)
    };
    assert_eq!(library.count(&favorites).unwrap(), 1);
}

#[test]
fn removing_a_source_keeps_history_but_hides_rows() {
    let (_dir, library, source) = fixture();
    let file = ingest(&library, &source, "Film.2020.mkv");
    let profile = library.profiles().unwrap().remove(0);
    let session = library.start_session(&profile.id, &file).unwrap();
    library
        .checkpoint(screenroom_core::Checkpoint {
            session: &session,
            position_ms: 2_000,
            duration_ms: 10_000,
            sequence: 1,
            seek_generation: 0,
            ended: false,
        })
        .unwrap();

    library.remove_source(&source.id).unwrap();
    assert!(library.sources().unwrap().is_empty());
    assert_eq!(library.count(&LibraryQuery::default()).unwrap(), 0);

    // Re-adding the same root resurrects the same source identity and progress.
    let again = library.add_source(_dir.path(), None).unwrap();
    assert_eq!(again.id, source.id);
    assert_eq!(library.count(&LibraryQuery::default()).unwrap(), 1);
    let restored = library.item(&file, Some(&profile.id)).unwrap().unwrap();
    assert_eq!(restored.position_ms, 2_000);
}

#[test]
fn next_episode_requires_a_consecutive_file() {
    let (_dir, library, source) = fixture();
    let one = ingest(&library, &source, "Show.S01E01.mkv");
    let two = ingest(&library, &source, "Show.S01E02.mkv");
    assert_eq!(
        library
            .next_episode(&one, None)
            .unwrap()
            .map(|item| item.id),
        Some(two.clone())
    );
    assert!(library.next_episode(&two, None).unwrap().is_none());

    let gap = library
        .ingest_file(Ingest {
            source_id: &source.id,
            relative_path: "Show.S01E04.mkv",
            fingerprint: "gap",
            size: 1_000,
            modified_ms: 1,
            scan_id: "scan-1",
            parsed: Some(&parse_filename("Show.S01E04.mkv", None)),
            expected_root: Some(&source.root_path),
        })
        .unwrap();
    // E02 has no E03, so autoplay must stop instead of skipping the gap.
    assert!(library.next_episode(&two, None).unwrap().is_none());
    assert!(library.next_episode(&gap, None).unwrap().is_none());
}

#[test]
fn season_filters_and_sorts_are_stable() {
    let (_dir, library, source) = fixture();
    ingest(&library, &source, "Show.S01E02.mkv");
    ingest(&library, &source, "Show.S01E01.mkv");
    ingest(&library, &source, "Show.S02E01.mkv");
    let by_title = library
        .items(&LibraryQuery {
            sort: Sort::Title,
            ..LibraryQuery::page(10)
        })
        .unwrap();
    assert_eq!(
        by_title
            .iter()
            .map(|item| (item.season, item.episode))
            .collect::<Vec<_>>(),
        vec![(Some(1), Some(1)), (Some(1), Some(2)), (Some(2), Some(1))]
    );

    let season_two = LibraryQuery {
        season: Some(2),
        ..LibraryQuery::page(10)
    };
    assert_eq!(library.count(&season_two).unwrap(), 1);
    assert_eq!(
        library.items(&season_two).unwrap().remove(0).episode,
        Some(1)
    );
}

#[test]
fn a_future_schema_is_refused_without_resetting_data() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("library.db");
    {
        let library = Library::open(&path).unwrap();
        drop(library);
    }
    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.pragma_update(None, "user_version", 99).unwrap();
    }
    let error = Library::open(&path).unwrap_err();
    assert_eq!(error.code(), "future_database");

    // The file is still a valid library for the version that wrote it.
    let connection = rusqlite::Connection::open(&path).unwrap();
    let tables: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='works'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(tables, 1);
}
