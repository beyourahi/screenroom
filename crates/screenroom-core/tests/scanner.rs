//! Scanner contract tests against a real temporary directory tree.

use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, SystemTime};

use screenroom_core::{Library, LibraryQuery, LibraryScanner, ScanToken};
use tempfile::TempDir;

/// Writes `contents` and backdates the mtime so the settle window passes.
fn write_settled(path: &Path, contents: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
    let file = File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(3600))
        .unwrap();
}

fn scanner(library: &Library) -> LibraryScanner<'_> {
    LibraryScanner::new(library).with_limits(10_000, 32, Duration::from_secs(15))
}

#[test]
fn scans_media_and_reports_skipped_entries() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write_settled(&root.join("Movies/Film.2020.1080p.mkv"), b"movie");
    write_settled(&root.join("Show/Season 1/Show.S01E01.mkv"), b"episode one");
    write_settled(&root.join("Show/Season 1/Show.S01E02.mkv"), b"episode two");
    write_settled(&root.join("notes.txt"), b"not media");
    write_settled(&root.join(".hidden/Secret.2020.mkv"), b"hidden");

    let library = Library::open_in_memory().unwrap();
    let source = library.add_source(root, None).unwrap();
    let token = ScanToken::new();
    let summary = scanner(&library).scan(&source.id, &token, |_| {}).unwrap();

    assert_eq!(summary.imported, 3);
    assert!(!summary.cancelled);
    assert!(summary.complete, "issues: {:?}", summary.issues);
    assert_eq!(library.count(&LibraryQuery::default()).unwrap(), 3);

    let show = LibraryQuery {
        kind: Some("series".into()),
        ..LibraryQuery::page(10)
    };
    let episodes = library.items(&show).unwrap();
    assert_eq!(episodes.len(), 2);
    assert_eq!(episodes[0].title, "Show");
    assert_eq!(episodes[0].season, Some(1));
    assert!(episodes.iter().all(|item| item.available));
}

#[test]
fn young_files_and_download_sidecars_are_not_imported() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    // Fresh mtime: still being written.
    fs::write(root.join("Fresh.2020.mkv"), b"growing").unwrap();
    // Settled but with a download sidecar.
    write_settled(&root.join("Queued.2020.mkv"), b"queued");
    fs::write(root.join("Queued.2020.mkv.part"), b"partial").unwrap();

    let library = Library::open_in_memory().unwrap();
    let source = library.add_source(root, None).unwrap();
    let summary = scanner(&library)
        .scan(&source.id, &ScanToken::new(), |_| {})
        .unwrap();

    assert_eq!(summary.imported, 0);
    assert!(!summary.complete);
    let codes: Vec<_> = summary.issues.iter().map(|issue| issue.code).collect();
    assert!(codes.contains(&"still_downloading"), "codes: {codes:?}");
    assert_eq!(library.count(&LibraryQuery::default()).unwrap(), 0);
}

#[test]
fn cancellation_stops_the_walk_and_keeps_availability() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    for index in 0..5 {
        write_settled(&root.join(format!("Film.{index}.2020.mkv")), b"data");
    }
    let library = Library::open_in_memory().unwrap();
    let source = library.add_source(root, None).unwrap();
    scanner(&library)
        .scan(&source.id, &ScanToken::new(), |_| {})
        .unwrap();
    assert_eq!(library.count(&LibraryQuery::default()).unwrap(), 5);

    fs::remove_file(root.join("Film.0.2020.mkv")).unwrap();
    let token = ScanToken::new();
    token.cancel();
    let summary = scanner(&library).scan(&source.id, &token, |_| {}).unwrap();
    assert!(summary.cancelled);
    // A cancelled scan must not withdraw availability.
    assert_eq!(library.count(&LibraryQuery::default()).unwrap(), 5);
}

#[test]
fn complete_rescan_withdraws_availability_without_losing_progress() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write_settled(&root.join("Keep.2020.mkv"), b"keep");
    write_settled(&root.join("Gone.2020.mkv"), b"gone");

    let library = Library::open_in_memory().unwrap();
    let source = library.add_source(root, None).unwrap();
    scanner(&library)
        .scan(&source.id, &ScanToken::new(), |_| {})
        .unwrap();
    assert_eq!(library.count(&LibraryQuery::default()).unwrap(), 2);

    let profile = library.profiles().unwrap().remove(0);
    let gone = library
        .items(&LibraryQuery::page(10))
        .unwrap()
        .into_iter()
        .find(|item| item.filename == "Gone.2020.mkv")
        .unwrap();
    library
        .set_favorite(&profile.id, &gone.work_id, true)
        .unwrap();

    fs::remove_file(root.join("Gone.2020.mkv")).unwrap();
    let summary = scanner(&library)
        .scan(&source.id, &ScanToken::new(), |_| {})
        .unwrap();
    assert!(summary.complete);

    let offline = library.item(&gone.id, Some(&profile.id)).unwrap().unwrap();
    assert!(!offline.available);
    assert!(offline.favorite);
    // Rows stay visible: an absent drive is not a deleted library.
    assert_eq!(library.count(&LibraryQuery::default()).unwrap(), 2);
    let only_available = LibraryQuery {
        available_only: true,
        ..LibraryQuery::page(10)
    };
    assert_eq!(library.count(&only_available).unwrap(), 1);
}

#[test]
fn symlinks_are_never_followed() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("root");
    fs::create_dir_all(&root).unwrap();
    let outside = dir.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    write_settled(&outside.join("Elsewhere.2020.mkv"), b"elsewhere");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();

    let library = Library::open_in_memory().unwrap();
    let source = library.add_source(&root, None).unwrap();
    let summary = scanner(&library)
        .scan(&source.id, &ScanToken::new(), |_| {})
        .unwrap();

    assert_eq!(summary.imported, 0);
    assert_eq!(library.count(&LibraryQuery::default()).unwrap(), 0);
}

#[test]
fn a_source_cannot_be_scanned_twice_at_once() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write_settled(&root.join("Film.2020.mkv"), b"data");
    let library = Library::open_in_memory().unwrap();
    let source = library.add_source(root, None).unwrap();

    // Attempt a nested scan from inside the first scan: the source is claimed.
    let mut nested_error = None;
    scanner(&library)
        .scan(&source.id, &ScanToken::new(), |update| {
            if !update.done {
                nested_error = Some(
                    LibraryScanner::new(&library)
                        .scan(&source.id, &ScanToken::new(), |_| {})
                        .unwrap_err(),
                );
            }
        })
        .unwrap();
    assert_eq!(
        nested_error.map(|error| error.code().to_string()),
        Some("scan_running".into())
    );

    // The claim is released, so the source is scannable again.
    assert!(
        scanner(&library)
            .scan(&source.id, &ScanToken::new(), |_| {})
            .is_ok()
    );
}

#[test]
fn progress_streams_while_scanning() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    for index in 0..3 {
        write_settled(&root.join(format!("Film.{index}.2020.mkv")), b"data");
    }
    let library = Library::open_in_memory().unwrap();
    let source = library.add_source(root, None).unwrap();
    let mut updates = 0;
    let summary = scanner(&library)
        .scan(&source.id, &ScanToken::new(), |update| {
            if !update.done {
                updates += 1;
            }
        })
        .unwrap();
    assert_eq!(summary.imported, 3);
    assert_eq!(updates, 3);
}
