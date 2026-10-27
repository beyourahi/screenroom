//! Recovery: interrupted work, refused migrations, and a damaged file.
//!
//! These are the paths a user reaches only after something has already gone wrong,
//! which is exactly why they are tested rather than assumed. The claim under test
//! throughout is that nothing is deleted and the reason is stated: a crash must not
//! cost watch history, and a file this build cannot read must be reported rather
//! than silently replaced by an empty one.

use std::fs;

use rusqlite::Connection;
use screenroom_core::{Ingest, Library, LibraryQuery, parse_filename};
use tempfile::TempDir;

/// Opens the database directly, to read state the public API does not expose.
///
/// A second connection to the same file is how these tests observe the rows a
/// crash left behind; reading them through the library's own API would test the
/// API against itself.
fn raw(directory: &TempDir) -> Connection {
    Connection::open(directory.path().join("library.db")).expect("reading connection")
}

/// Reads one integer from the database directly.
fn raw_count(directory: &TempDir, sql: &str) -> i64 {
    raw(directory)
        .query_row(sql, [], |row| row.get(0))
        .expect(sql)
}

/// A file-backed library plus one ingested, readable media file.
fn library_with_a_file(directory: &TempDir) -> (Library, String) {
    let media = directory.path().join("media");
    fs::create_dir_all(&media).unwrap();
    let library = Library::open(directory.path().join("library.db")).expect("library");
    let source = library.add_source(&media, None).expect("source");
    let name = "Heat (1995).mkv";
    fs::write(media.join(name), b"not really a video").unwrap();
    let parsed = parse_filename(name, None);
    let file_id = library
        .ingest_file(Ingest {
            source_id: &source.id,
            relative_path: name,
            fingerprint: "fingerprint",
            size: 18,
            modified_ms: 1,
            scan_id: "test",
            parsed: Some(&parsed),
            expected_root: Some(&source.root_path),
        })
        .expect("ingest");
    (library, file_id)
}

#[test]
fn a_playback_session_open_when_the_process_died_is_closed_on_reopen() {
    let directory = TempDir::new().expect("temp dir");
    let (library, file_id) = library_with_a_file(&directory);
    let profile = library.profiles().unwrap().remove(0);

    // A session that is opened, written to, and then abandoned: the process ends
    // here without `end_session` ever running.
    let session = library
        .start_session(&profile.id, &file_id)
        .expect("session");
    library
        .checkpoint(screenroom_core::Checkpoint {
            session: &session,
            position_ms: 12_000,
            duration_ms: 60_000,
            sequence: 1,
            seek_generation: 0,
            ended: false,
        })
        .expect("checkpoint");
    const ACTIVE: &str = "SELECT COUNT(*) FROM sessions WHERE status = 'active'";
    assert_eq!(
        raw_count(&directory, ACTIVE),
        1,
        "the session is active before the reopen"
    );

    let reopened = Library::open(directory.path().join("library.db")).expect("reopen");
    assert_eq!(
        raw_count(&directory, ACTIVE),
        0,
        "a crashed process cannot stay authoritative for a session"
    );

    // The whole point: the viewing is still resumable.
    let watching = reopened
        .continue_watching(&profile.id, 5)
        .expect("continue");
    assert_eq!(
        watching.len(),
        1,
        "the interrupted viewing is still offered"
    );
    assert_eq!(
        watching[0].position_ms, 12_000,
        "at the position it reached"
    );
}

#[test]
fn a_job_that_was_running_when_the_process_died_is_queued_again() {
    let directory = TempDir::new().expect("temp dir");
    let (_library, _file_id) = library_with_a_file(&directory);
    let database = directory.path().join("library.db");
    {
        // Written directly because no first-party caller creates jobs yet; the
        // guarantee under test belongs to opening the library, not to the writer.
        let connection = Connection::open(&database).unwrap();
        connection
            .execute(
                "INSERT INTO jobs(id, kind, target, status, priority, attempts, next_retry_ms)
                 VALUES('job-1', 'probe', 'target-1', 'running', 0, 1, 0)",
                [],
            )
            .unwrap();
    }

    let _reopened = Library::open(&database).expect("reopen");
    let status: String = Connection::open(&database)
        .unwrap()
        .query_row("SELECT status FROM jobs WHERE id = 'job-1'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        status, "queued",
        "interrupted work is retried, not left claiming to run"
    );
}

#[test]
fn an_unsupported_schema_is_refused_without_resetting_the_data() {
    let directory = TempDir::new().expect("temp dir");
    let database = directory.path().join("library.db");
    {
        let connection = Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE marker(text TEXT);
                 INSERT INTO marker(text) VALUES('the visitor');
                 PRAGMA user_version = 1;",
            )
            .unwrap();
    }

    let error = Library::open(&database).unwrap_err();
    assert_eq!(error.code(), "migration_unavailable");
    let survived: String = Connection::open(&database)
        .unwrap()
        .query_row("SELECT text FROM marker", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        survived, "the visitor",
        "a refused migration must leave the file exactly as it was"
    );
}

#[test]
fn a_future_schema_is_refused_and_named() {
    let directory = TempDir::new().expect("temp dir");
    let database = directory.path().join("library.db");
    {
        let connection = Connection::open(&database).unwrap();
        connection
            .execute_batch("PRAGMA user_version = 99;")
            .unwrap();
    }

    let error = Library::open(&database).unwrap_err();
    assert_eq!(error.code(), "future_database");
    assert!(
        error.to_string().contains("99"),
        "the refusal names the version it found: {error}"
    );
}

#[test]
fn a_damaged_database_is_reported_rather_than_replaced() {
    let directory = TempDir::new().expect("temp dir");
    let database = directory.path().join("library.db");
    // A page-sized run of garbage: enough that SQLite accepts the header area and
    // then fails its own integrity check, which is the case the user hits after a
    // filesystem error.
    fs::write(&database, vec![0x7f; 64 * 1024]).unwrap();
    let before = fs::read(&database).unwrap();

    let error = Library::open(&database).unwrap_err();
    assert!(
        matches!(error.code(), "corrupt_database" | "database_error"),
        "unexpected code {}",
        error.code()
    );
    assert_eq!(
        fs::read(&database).unwrap(),
        before,
        "a damaged library is never overwritten by an empty one"
    );
}

#[test]
fn a_database_with_no_schema_is_initialised_rather_than_refused() {
    let directory = TempDir::new().expect("temp dir");
    let database = directory.path().join("library.db");
    // A zero-byte file is what a first run leaves behind, and what an interrupted
    // creation leaves behind. It must become a usable library.
    fs::write(&database, b"").unwrap();

    let library = Library::open(&database).expect("a fresh library");
    assert_eq!(library.profiles().unwrap().len(), 1);
    assert_eq!(library.items(&LibraryQuery::page(10)).unwrap().len(), 0);
    assert!(database.metadata().unwrap().len() > 0);
}

#[test]
fn progress_survives_the_whole_recovery_sequence() {
    let directory = TempDir::new().expect("temp dir");
    let (library, file_id) = library_with_a_file(&directory);
    let profile = library.profiles().unwrap().remove(0);
    library
        .set_favorite(
            &profile.id,
            &library.item(&file_id, None).unwrap().unwrap().work_id,
            true,
        )
        .expect("favorite");
    let session = library
        .start_session(&profile.id, &file_id)
        .expect("session");
    library
        .checkpoint(screenroom_core::Checkpoint {
            session: &session,
            position_ms: 30_000,
            duration_ms: 60_000,
            sequence: 1,
            seek_generation: 0,
            ended: false,
        })
        .expect("checkpoint");
    // No `end_session`: the process is treated as having died here.
    drop(session);
    drop(library);

    // Reopen several times, as a user relaunching after each crash would.
    for _ in 0..3 {
        let reopened = Library::open(directory.path().join("library.db")).expect("reopen");
        let watching = reopened
            .continue_watching(&profile.id, 5)
            .expect("continue");
        assert_eq!(watching.len(), 1);
        assert_eq!(watching[0].position_ms, 30_000, "the position is stable");
        assert!(watching[0].favorite, "and so is the list entry");
    }
}
