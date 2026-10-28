//! Scanner contract tests against a real temporary directory tree.

use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, SystemTime};

use screenroom_core::{Library, LibraryQuery, LibraryScanner, ScanToken, SourceState};
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
fn apple_double_companions_are_not_media() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    // What macOS writes beside every file on an exFAT USB drive: 4 KiB of
    // metadata with the same name and the same media extension.
    write_settled(&root.join("Film.2020.mkv"), b"real film");
    write_settled(&root.join("._Film.2020.mkv"), b"apple double");
    write_settled(&root.join("Show/Season 1/Show.S01E01.mkv"), b"real episode");
    write_settled(
        &root.join("Show/Season 1/._Show.S01E01.mkv"),
        b"apple double",
    );

    let library = Library::open_in_memory().unwrap();
    let source = library.add_source(root, None).unwrap();
    let summary = scanner(&library)
        .scan(&source.id, &ScanToken::new(), |_| {})
        .unwrap();

    assert_eq!(summary.imported, 2, "only the two real files");
    assert!(summary.complete, "issues: {:?}", summary.issues);
    let items = library.items(&LibraryQuery::page(10)).unwrap();
    assert_eq!(items.len(), 2);
    assert!(
        items.iter().all(|item| !item.title.starts_with("._")),
        "none of {:?} is an AppleDouble companion",
        items.iter().map(|item| &item.title).collect::<Vec<_>>()
    );
}

#[test]
fn two_shows_with_one_title_and_different_years_stay_separate() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    // Same title, different shows. Episode filenames carry no year, so the year
    // can only come from the show folder — and it is what keeps them apart.
    write_settled(
        &root.join("The Office (2001)/Season 01/The Office - s01e01.mkv"),
        b"british",
    );
    write_settled(
        &root.join("The Office (2005)/Season 01/The Office - s01e01.mkv"),
        b"american",
    );

    let library = Library::open_in_memory().unwrap();
    let source = library.add_source(root, None).unwrap();
    scanner(&library)
        .scan(&source.id, &ScanToken::new(), |_| {})
        .unwrap();

    let episodes = library
        .items(&LibraryQuery {
            kind: Some("series".into()),
            ..LibraryQuery::page(10)
        })
        .unwrap();
    assert_eq!(episodes.len(), 2, "two episodes, one per show");
    assert_ne!(episodes[0].work_id, episodes[1].work_id, "two works");
    let mut years: Vec<_> = episodes.iter().filter_map(|item| item.year).collect();
    years.sort();
    assert_eq!(years, vec![2001, 2005]);
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
fn an_unplugged_drive_is_not_an_empty_library() {
    // The worst failure this application could have is treating a missing disk as a
    // deleted library: a rescan of an unplugged drive must not withdraw availability,
    // drop rows, or lose progress. It must report the source as unavailable and
    // report the scan as incomplete, so nothing downstream believes it finished.
    //
    // A volume that is gone and a folder that was deleted are both covered, because
    // they carry different recovery copy and the same preservation guarantees.
    let dir = TempDir::new().unwrap();
    let volume = dir.path().join("volume");
    let root = volume.join("drive");
    fs::create_dir_all(&root).unwrap();
    write_settled(&root.join("Film.2020.mkv"), b"film");
    write_settled(&root.join("Show.S01E01.mkv"), b"episode");

    let library = Library::open_in_memory().unwrap();
    let source = library.add_source(&root, None).unwrap();
    scanner(&library)
        .scan(&source.id, &ScanToken::new(), |_| {})
        .unwrap();
    assert_eq!(library.count(&LibraryQuery::default()).unwrap(), 2);

    let profile = library.profiles().unwrap().remove(0);
    let file = library
        .items(&LibraryQuery::page(10))
        .unwrap()
        .into_iter()
        .find(|item| item.filename == "Film.2020.mkv")
        .unwrap();
    library
        .set_favorite(&profile.id, &file.work_id, true)
        .unwrap();
    let file_id = file.id.clone();

    // The folder is deleted while its volume stays mounted. The recovery is a
    // relink, so saying "drive not attached" here would send the owner to do
    // something that cannot work.
    fs::remove_dir_all(&root).unwrap();
    let summary = scanner(&library)
        .scan(&source.id, &ScanToken::new(), |_| {})
        .unwrap();

    assert!(
        !summary.complete,
        "a scan that could not read its root must not claim to have completed"
    );
    assert_eq!(summary.imported, 0);
    assert_eq!(summary.visited, 0);
    assert_eq!(
        summary.issues.first().map(|issue| issue.code),
        Some("source_missing"),
        "a deleted folder is not an unmounted drive"
    );
    assert_eq!(
        library.source(&source.id).unwrap().state,
        SourceState::Missing
    );

    // Everything the owner had is still there, and still theirs.
    assert_eq!(library.count(&LibraryQuery::default()).unwrap(), 2);
    let offline = library.item(&file_id, Some(&profile.id)).unwrap().unwrap();
    assert!(!offline.available);
    assert!(offline.favorite, "a list entry was lost with the drive");
    let favorites = LibraryQuery {
        profile_id: Some(profile.id.clone()),
        favorites_only: true,
        ..LibraryQuery::page(10)
    };
    assert_eq!(library.count(&favorites).unwrap(), 1);

    // And the folder coming back restores availability rather than leaving it stale.
    fs::create_dir_all(&root).unwrap();
    write_settled(&root.join("Film.2020.mkv"), b"film");
    write_settled(&root.join("Show.S01E01.mkv"), b"episode");
    let again = scanner(&library)
        .scan(&source.id, &ScanToken::new(), |_| {})
        .unwrap();
    assert!(again.complete);
    assert!(
        library
            .item(&file_id, Some(&profile.id))
            .unwrap()
            .unwrap()
            .available,
        "reconnecting the drive did not restore availability"
    );
    assert_eq!(library.count(&LibraryQuery::default()).unwrap(), 2);

    // Now the volume itself goes away, taking the mount point with it. This one is
    // a disconnected drive, and the recovery is to attach it again.
    fs::remove_dir_all(&volume).unwrap();
    let unplugged = scanner(&library)
        .scan(&source.id, &ScanToken::new(), |_| {})
        .unwrap();
    assert!(!unplugged.complete);
    assert_eq!(
        unplugged.issues.first().map(|issue| issue.code),
        Some("source_disconnected")
    );
    assert_eq!(
        library.source(&source.id).unwrap().state,
        SourceState::Disconnected
    );
    assert_eq!(
        library.count(&LibraryQuery::default()).unwrap(),
        2,
        "an unmounted volume withdrew rows"
    );
    assert!(
        library
            .item(&file_id, Some(&profile.id))
            .unwrap()
            .unwrap()
            .favorite,
        "an unmounted volume lost a list entry"
    );
}

#[test]
fn a_root_behind_a_withdrawn_grant_is_not_a_missing_drive() {
    // Access being revoked is a third case with its own recovery: re-granting. It is
    // the one the owner cannot fix by looking at cables, so conflating it with the
    // others would waste their time.
    //
    // Skipped when the process can read a mode-000 directory anyway, which is the
    // case for a root user; the assertion is about the classification the operating
    // system reports, not about this test being able to create the condition.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let locked = dir.path().join("locked");
        let root = locked.join("media");
        fs::create_dir_all(&root).unwrap();
        write_settled(&root.join("Film.2020.mkv"), b"film");

        let library = Library::open_in_memory().unwrap();
        let source = library.add_source(&root, None).unwrap();
        scanner(&library)
            .scan(&source.id, &ScanToken::new(), |_| {})
            .unwrap();

        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let denied = fs::read_dir(&locked).is_err();
        let outcome = denied.then(|| {
            scanner(&library)
                .scan(&source.id, &ScanToken::new(), |_| {})
                .unwrap()
        });
        // Restore before asserting, so a failure cannot leave an unreadable
        // directory behind for the temporary-directory cleanup.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

        if let Some(summary) = outcome {
            assert_eq!(
                summary.issues.first().map(|issue| issue.code),
                Some("permission_lost")
            );
            assert_eq!(
                library.source(&source.id).unwrap().state,
                SourceState::PermissionLost
            );
            assert_eq!(
                library.count(&LibraryQuery::default()).unwrap(),
                1,
                "a withdrawn grant withdrew rows"
            );
        }
    }
}

#[test]
fn a_root_replaced_by_a_symlink_is_refused_rather_than_followed() {
    // A root whose canonical path no longer matches what was authorized is a
    // relink question, not a scan question: following it would read media the owner
    // never granted.
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("root");
    fs::create_dir_all(&root).unwrap();
    write_settled(&root.join("Inside.2020.mkv"), b"inside");

    let library = Library::open_in_memory().unwrap();
    let source = library.add_source(&root, None).unwrap();
    scanner(&library)
        .scan(&source.id, &ScanToken::new(), |_| {})
        .unwrap();

    let elsewhere = dir.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    write_settled(&elsewhere.join("Outside.2020.mkv"), b"outside");
    fs::remove_dir_all(&root).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&elsewhere, &root).unwrap();

    #[cfg(unix)]
    {
        let error = scanner(&library)
            .scan(&source.id, &ScanToken::new(), |_| {})
            .unwrap_err();
        assert_eq!(error.code(), "unsafe_path");
        // Nothing outside the granted root was imported, and nothing was withdrawn.
        assert_eq!(library.count(&LibraryQuery::default()).unwrap(), 1);
        assert!(
            !library
                .items(&LibraryQuery::page(10))
                .unwrap()
                .iter()
                .any(|item| item.filename == "Outside.2020.mkv"),
            "media outside the authorized root was read"
        );
    }
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
