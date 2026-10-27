//! Preferences, backup and restore.
//!
//! These are the tests that stand between the user's history and a bad write, so
//! they check the refusals as carefully as the happy path: what happens to an
//! unrelated file, to the library file itself, and to the previous library after
//! a restore.

use std::fs;

use screenroom_core::{Ingest, Library, Preferences, parse_filename};
use tempfile::TempDir;

/// A file-backed library, because backup and restore are about the file on disk.
fn open_library(directory: &TempDir) -> Library {
    Library::open(directory.path().join("screenroom.db")).expect("library")
}

fn ingest(library: &Library, source_id: &str, root: &str, path: &str) -> String {
    let parsed = parse_filename(path, None);
    library
        .ingest_file(Ingest {
            source_id,
            relative_path: path,
            fingerprint: &format!("fingerprint:{path}"),
            size: 1_000,
            modified_ms: 1,
            scan_id: "scan-1",
            parsed: Some(&parsed),
            expected_root: Some(root),
        })
        .expect("ingest")
}

/// A library with media, a list entry and stored progress.
fn populated(directory: &TempDir) -> (Library, String) {
    let media = directory.path().join("media");
    fs::create_dir_all(&media).unwrap();
    let library = open_library(directory);
    let source = library.add_source(&media, None).expect("source");
    let first = ingest(&library, &source.id, &source.root_path, "Heat (1995).mkv");
    ingest(&library, &source.id, &source.root_path, "Alien (1979).mkv");
    let profile = library.profiles().unwrap().remove(0);
    library
        .set_favorite(
            &profile.id,
            &library.item(&first, None).unwrap().unwrap().work_id,
            true,
        )
        .expect("favorite");
    let session = library.start_session(&profile.id, &first).expect("session");
    library
        .checkpoint(screenroom_core::Checkpoint {
            session: &session,
            position_ms: 4_200,
            duration_ms: 60_000,
            sequence: 1,
            seek_generation: 0,
            ended: false,
        })
        .expect("checkpoint");
    library.end_session(&session).expect("end session");
    (library, source.id)
}

#[test]
fn preferences_start_with_defensible_defaults() {
    let (_directory, library, _source) = {
        let directory = TempDir::new().unwrap();
        let library = open_library(&directory);
        let source = library.add_source(directory.path(), None).unwrap();
        (directory, library, source)
    };
    let preferences = library.preferences().expect("preferences");
    assert_eq!(preferences, Preferences::default());
    assert!(
        preferences.autoplay_next,
        "a series should continue by default: the setting is one toggle away"
    );
    assert!(preferences.subtitle_language.is_none());
    assert_eq!(preferences.subtitle_scale_percent, 100);
    assert_eq!(preferences.subtitle_position_percent, 100);
}

#[test]
fn a_subtitle_style_outside_the_offered_range_is_refused() {
    let directory = TempDir::new().expect("temp dir");
    let library = open_library(&directory);
    for (preferences, code) in [
        (
            Preferences {
                subtitle_scale_percent: 1,
                ..Preferences::default()
            },
            "subtitle_scale_out_of_range",
        ),
        (
            Preferences {
                subtitle_scale_percent: 301,
                ..Preferences::default()
            },
            "subtitle_scale_out_of_range",
        ),
        (
            Preferences {
                subtitle_position_percent: 101,
                ..Preferences::default()
            },
            "subtitle_position_out_of_range",
        ),
    ] {
        let error = library.set_preferences(&preferences).unwrap_err();
        assert_eq!(error.code(), code);
    }
    // Refused on the way in, so the previously stored style is still what opens.
    assert_eq!(library.preferences().unwrap(), Preferences::default());
}

#[test]
fn an_unusable_stored_subtitle_style_still_lets_the_library_open() {
    // A library written by a build with a wider range, or corrupted, must open.
    // The read direction clamps and falls back; only the write direction refuses.
    let directory = TempDir::new().expect("temp dir");
    let library = open_library(&directory);
    library
        .set_setting("subtitleScalePercent", "9000")
        .expect("inject a wide value");
    library
        .set_setting("subtitlePositionPercent", "not a number")
        .expect("inject a corrupt value");

    let preferences = library.preferences().expect("preferences");
    assert_eq!(
        preferences.subtitle_scale_percent,
        Preferences::SUBTITLE_SCALE_MAX_PERCENT
    );
    assert_eq!(
        preferences.subtitle_position_percent,
        Preferences::default().subtitle_position_percent
    );
}

#[test]
fn preferences_survive_a_reopen() {
    let directory = TempDir::new().expect("temp dir");
    let library = open_library(&directory);
    let profile = library.profiles().unwrap().remove(0);
    let chosen = Preferences {
        autoplay_next: false,
        subtitle_language: Some("pt-BR".to_string()),
        audio_language: Some("en".to_string()),
        subtitle_scale_percent: 175,
        subtitle_position_percent: 92,
        default_profile: Some(profile.id.clone()),
    };
    library.set_preferences(&chosen).expect("save");

    let reopened = open_library(&directory);
    assert_eq!(reopened.preferences().expect("preferences"), chosen);
}

#[test]
fn clearing_a_language_preference_removes_it() {
    let directory = TempDir::new().expect("temp dir");
    let library = open_library(&directory);
    library
        .set_preferences(&Preferences {
            subtitle_language: Some("en".to_string()),
            ..Preferences::default()
        })
        .expect("save");
    library
        .set_preferences(&Preferences::default())
        .expect("clear");

    assert!(library.setting("subtitleLanguage").unwrap().is_none());
    assert_eq!(library.preferences().unwrap().subtitle_language, None);
}

#[test]
fn an_injected_language_tag_is_refused_and_nothing_is_saved() {
    let directory = TempDir::new().expect("temp dir");
    let library = open_library(&directory);
    let error = library
        .set_preferences(&Preferences {
            autoplay_next: false,
            subtitle_language: Some("en'; DROP TABLE progress; --".to_string()),
            ..Preferences::default()
        })
        .unwrap_err();
    assert_eq!(error.code(), "invalid_language");
    // The rejection is for the whole save, so the other field did not land either.
    assert_eq!(library.preferences().unwrap(), Preferences::default());
}

#[test]
fn a_default_profile_that_does_not_exist_is_refused() {
    let directory = TempDir::new().expect("temp dir");
    let library = open_library(&directory);
    let error = library
        .set_preferences(&Preferences {
            default_profile: Some("no-such-profile".to_string()),
            ..Preferences::default()
        })
        .unwrap_err();
    assert_eq!(error.code(), "profile_missing");
}

#[test]
fn a_key_this_build_does_not_model_is_preserved() {
    let directory = TempDir::new().expect("temp dir");
    let library = open_library(&directory);
    library.set_setting("fromANewerBuild", "keep me").unwrap();
    library
        .set_preferences(&Preferences {
            autoplay_next: false,
            ..Preferences::default()
        })
        .expect("save");
    assert_eq!(
        library.setting("fromANewerBuild").unwrap().as_deref(),
        Some("keep me")
    );
}

#[test]
fn a_backup_carries_every_table_and_restores_into_a_fresh_library() {
    let directory = TempDir::new().expect("temp dir");
    let backup_path = directory.path().join("backups").join("nightly.db");
    let (library, _source) = populated(&directory);
    let report = library.backup_to(&backup_path).expect("backup");

    assert!(report.path.ends_with("nightly.db"));
    assert!(report.bytes > 0, "a backup with content is not empty");
    assert_eq!(report.info.works, 2, "both films are in the backup");
    assert_eq!(report.info.files, 2);
    assert_eq!(report.info.sources, 1);
    assert_eq!(report.info.profiles, 1);
    assert!(report.info.created_ms.is_some(), "the backup is dated");

    // A different, empty library restored from it holds the same content.
    let restore_directory = TempDir::new().expect("temp dir");
    let target = open_library(&restore_directory);
    target.restore_from(&backup_path).expect("restore");

    let query = screenroom_core::LibraryQuery::page(50);
    let restored = target.items(&query).expect("items");
    let original = library.items(&query).expect("items");
    assert_eq!(restored.len(), original.len());
    assert_eq!(target.profiles().unwrap().len(), 1);
    let profile = target.profiles().unwrap().remove(0);
    assert_eq!(
        target.continue_watching(&profile.id, 5).unwrap().len(),
        1,
        "stored progress came across"
    );
}

#[test]
fn a_restored_library_does_not_keep_the_backup_stamp() {
    let directory = TempDir::new().expect("temp dir");
    let (library, _source) = populated(&directory);
    let backup_path = directory.path().join("copy.db");
    library.backup_to(&backup_path).expect("backup");
    library.restore_from(&backup_path).expect("restore");

    assert!(
        library.setting("backupFormatVersion").unwrap().is_none(),
        "the live library must not describe itself as a backup"
    );
    assert!(library.setting("backupCreatedMs").unwrap().is_none());
}

#[test]
fn a_restore_keeps_the_previous_library_beside_it() {
    let directory = TempDir::new().expect("temp dir");
    let (library, _source) = populated(&directory);
    let backup_path = directory.path().join("copy.db");
    library.backup_to(&backup_path).expect("backup");

    // Change the library after the backup, so the safety copy is distinguishable.
    let source = library.sources().unwrap().remove(0);
    ingest(&library, &source.id, &source.root_path, "Se7en (1995).mkv");
    assert_eq!(
        library
            .items(&screenroom_core::LibraryQuery::page(50))
            .unwrap()
            .len(),
        3
    );

    let report = library.restore_from(&backup_path).expect("restore");
    let safety = report
        .previous_saved_to
        .expect("a safety copy is written before replacing anything");
    assert!(std::path::Path::new(&safety).is_file());

    // The live library is back to the backup, and the safety copy holds the state
    // that was replaced, so the user can go back to it.
    assert_eq!(
        library
            .items(&screenroom_core::LibraryQuery::page(50))
            .unwrap()
            .len(),
        2
    );
    let previous = Library::open(&safety).expect("safety copy opens");
    assert_eq!(
        previous
            .items(&screenroom_core::LibraryQuery::page(50))
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn a_backup_refuses_to_write_over_the_library_itself() {
    let directory = TempDir::new().expect("temp dir");
    let (library, _source) = populated(&directory);
    let live = library.path().expect("file-backed library");
    let error = library.backup_to(&live).unwrap_err();
    assert_eq!(error.code(), "backup_is_library");
    assert!(
        library
            .items(&screenroom_core::LibraryQuery::page(50))
            .unwrap()
            .len()
            == 2,
        "the refused backup left the library readable"
    );
}

#[test]
fn a_backup_needs_an_absolute_path() {
    let directory = TempDir::new().expect("temp dir");
    let (library, _source) = populated(&directory);
    let error = library
        .backup_to(std::path::Path::new("relative.db"))
        .unwrap_err();
    assert_eq!(error.code(), "invalid_backup_path");
}

#[test]
fn a_missing_backup_is_reported_as_missing() {
    let directory = TempDir::new().expect("temp dir");
    let (library, _source) = populated(&directory);
    let error = library
        .restore_from(directory.path().join("nothing-here.db"))
        .unwrap_err();
    assert_eq!(error.code(), "backup_missing");
}

#[test]
fn a_file_that_is_not_a_database_is_refused() {
    let directory = TempDir::new().expect("temp dir");
    let (library, _source) = populated(&directory);
    let not_a_database = directory.path().join("holiday-photo.db");
    fs::write(&not_a_database, b"\x89PNG\r\n\x1a\nnot sqlite at all").unwrap();

    let error = library.restore_from(&not_a_database).unwrap_err();
    assert!(
        matches!(error.code(), "backup_unreadable" | "backup_corrupt"),
        "unexpected code {}",
        error.code()
    );
    assert_eq!(
        library
            .items(&screenroom_core::LibraryQuery::page(50))
            .unwrap()
            .len(),
        2,
        "a refused restore changes nothing"
    );
}

#[test]
fn a_database_that_is_not_a_screenroom_backup_is_refused() {
    let directory = TempDir::new().expect("temp dir");
    let (library, _source) = populated(&directory);
    let stranger = directory.path().join("someone-elses.db");
    {
        let connection = rusqlite::Connection::open(&stranger).unwrap();
        connection
            .execute_batch("CREATE TABLE notes(body TEXT); PRAGMA user_version = 2;")
            .unwrap();
    }

    let error = library.restore_from(&stranger).unwrap_err();
    assert_eq!(error.code(), "backup_unrecognised");
}

#[test]
fn a_backup_from_a_newer_format_is_refused() {
    let directory = TempDir::new().expect("temp dir");
    let (library, _source) = populated(&directory);
    let backup_path = directory.path().join("from-the-future.db");
    library.backup_to(&backup_path).expect("backup");

    {
        let connection = rusqlite::Connection::open(&backup_path).unwrap();
        connection
            .execute(
                "UPDATE settings SET value = '99' WHERE key = 'backupFormatVersion'",
                [],
            )
            .unwrap();
    }

    let error = Library::inspect_backup(&backup_path).unwrap_err();
    assert_eq!(error.code(), "backup_unsupported");
}

#[test]
fn inspecting_a_backup_does_not_change_it() {
    let directory = TempDir::new().expect("temp dir");
    let (library, _source) = populated(&directory);
    let backup_path = directory.path().join("copy.db");
    library.backup_to(&backup_path).expect("backup");
    let before = fs::metadata(&backup_path).unwrap().len();
    let before_modified = fs::metadata(&backup_path).unwrap().modified().unwrap();

    let info = Library::inspect_backup(&backup_path).expect("inspect");
    assert_eq!(info.works, 2);
    assert_eq!(info.format_version, screenroom_core::BACKUP_FORMAT_VERSION);

    let after = fs::metadata(&backup_path).unwrap();
    assert_eq!(before, after.len());
    assert_eq!(before_modified, after.modified().unwrap());
}

#[test]
fn a_failed_backup_leaves_no_partial_file_behind() {
    let directory = TempDir::new().expect("temp dir");
    let (library, _source) = populated(&directory);
    // A folder cannot be replaced by a file, so the final move fails.
    let blocked = directory.path().join("blocked");
    fs::create_dir(&blocked).unwrap();

    let error = library.backup_to(&blocked).unwrap_err();
    assert_eq!(error.code(), "backup_is_folder");
    let leftovers: Vec<_> = fs::read_dir(directory.path())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".partial"))
        .collect();
    assert!(leftovers.is_empty(), "left behind: {leftovers:?}");
}
