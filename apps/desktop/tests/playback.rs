//! Durable resume across a relaunch, and the checkpoint policy around it.
//!
//! The store-level tests use the real SQLite file so "reopen the library" means
//! exactly that. The end-to-end test additionally decodes a real clip with libmpv
//! and reports honestly when this host has no player.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use screenroom_core::{Ingest, Library, parse_filename};
use screenroom_desktop::playback::PlaybackTracker;
use screenroom_player::Player;
use tempfile::TempDir;

/// Creates a library on disk with one ingested file, and returns its path.
fn seeded(directory: &TempDir, relative_path: &str) -> (PathBuf, String) {
    let database = directory.path().join("library.db");
    let media_root = directory.path().join("media");
    fs::create_dir_all(&media_root).expect("media root");
    let file_path = media_root.join(relative_path);
    fs::write(&file_path, b"not-real-media").expect("placeholder file");

    let library = Library::open(&database).expect("library");
    let source = library.add_source(&media_root, None).expect("source");
    let parsed = parse_filename(relative_path, None);
    let file_id = library
        .ingest_file(Ingest {
            source_id: &source.id,
            relative_path,
            fingerprint: "test-fingerprint",
            size: 15,
            modified_ms: 1,
            scan_id: "scan-1",
            parsed: Some(&parsed),
            expected_root: Some(&source.root_path),
        })
        .expect("ingest");
    drop(library);
    (database, file_id)
}

fn profile(library: &Library) -> String {
    library.profiles().expect("profiles")[0].id.clone()
}

fn stored_position(database: &Path, file_id: &str) -> (i64, bool) {
    let library = Library::open(database).expect("reopened library");
    let profile = profile(&library);
    let item = library
        .item(file_id, Some(&profile))
        .expect("item")
        .expect("file present");
    (item.position_ms, item.watched)
}

#[test]
fn progress_survives_reopening_the_library() {
    let directory = TempDir::new().expect("temp dir");
    let (database, file_id) = seeded(&directory, "Film.2020.mkv");

    {
        let library = Library::open(&database).expect("library");
        let opened = PlaybackTracker::open(&library, &profile(&library), &file_id).expect("open");
        assert_eq!(opened.resume_from_ms, 0);
        let mut tracker = opened.tracker;
        assert!(
            tracker
                .record(&library, 30_000, 120_000, false)
                .expect("checkpoint")
        );
        tracker.close(&library).expect("close");
    }

    assert_eq!(stored_position(&database, &file_id), (30_000, false));

    // A second launch must resume where the first one stopped.
    let library = Library::open(&database).expect("library");
    let opened = PlaybackTracker::open(&library, &profile(&library), &file_id).expect("open");
    assert_eq!(opened.resume_from_ms, 30_000);
}

#[test]
fn a_finished_file_resumes_from_the_start() {
    let directory = TempDir::new().expect("temp dir");
    let (database, file_id) = seeded(&directory, "Film.2020.mkv");
    let library = Library::open(&database).expect("library");
    let mut tracker = PlaybackTracker::open(&library, &profile(&library), &file_id)
        .expect("open")
        .tracker;
    assert!(
        tracker
            .record(&library, 120_000, 120_000, true)
            .expect("checkpoint")
    );
    drop(library);

    let (position, watched) = stored_position(&database, &file_id);
    assert!(watched, "finished playback must mark the edition watched");
    assert_eq!(position, 120_000);

    let library = Library::open(&database).expect("library");
    let opened = PlaybackTracker::open(&library, &profile(&library), &file_id).expect("open");
    assert_eq!(opened.resume_from_ms, 0, "replaying from the end is wrong");
}

#[test]
fn checkpoints_are_throttled_but_a_seek_is_not() {
    let directory = TempDir::new().expect("temp dir");
    let (database, file_id) = seeded(&directory, "Film.2020.mkv");
    let library = Library::open(&database).expect("library");
    let mut tracker = PlaybackTracker::open(&library, &profile(&library), &file_id)
        .expect("open")
        .tracker;

    assert!(tracker.record(&library, 1_000, 60_000, false).unwrap());
    // Half a second later: too soon, and the core is not asked.
    assert!(!tracker.record(&library, 1_500, 60_000, false).unwrap());
    // A second later: due.
    assert!(tracker.record(&library, 2_200, 60_000, false).unwrap());

    // After a seek the next report belongs to a new generation, so it must be
    // written even though the position barely moved.
    tracker.seek(&library).expect("seek generation");
    assert!(tracker.record(&library, 2_300, 60_000, false).unwrap());
    tracker.close(&library).expect("close");
    assert_eq!(stored_position(&database, &file_id).0, 2_300);
}

#[test]
fn a_zero_duration_is_never_written() {
    let directory = TempDir::new().expect("temp dir");
    let (database, file_id) = seeded(&directory, "Film.2020.mkv");
    let library = Library::open(&database).expect("library");
    let mut tracker = PlaybackTracker::open(&library, &profile(&library), &file_id)
        .expect("open")
        .tracker;
    assert!(!tracker.record(&library, 5_000, 0, false).unwrap());
    assert!(!tracker.record(&library, -10, 60_000, false).unwrap());
    tracker.close(&library).expect("close");
    assert_eq!(stored_position(&database, &file_id).0, 0);
}

/// Writes a real, rights-clear clip with FFmpeg and returns its path.
///
/// `testsrc2` is generated inside this process's control and carries a burnt-in
/// timecode, so nothing here depends on the machine's own media.
fn generated_clip(media_root: &Path, name: &str, seconds: u32) -> PathBuf {
    let clip = media_root.join(name);
    let video = format!("testsrc2=size=320x240:rate=25:duration={seconds}");
    let audio = format!("sine=frequency=440:duration={seconds}");
    let status = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-y",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            &video,
            "-f",
            "lavfi",
            "-i",
            &audio,
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(&clip)
        .status()
        .expect("ffmpeg is required by the decode tests");
    assert!(
        status.success(),
        "ffmpeg could not write {}",
        clip.display()
    );
    clip
}

/// Waits until decoded frames are running past `seconds`.
///
/// A player that loads but never advances is the failure this reports, so the
/// last snapshot is part of the panic message.
fn wait_for_position(player: &Player, seconds: f64) -> screenroom_player::PlayerSnapshot {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let snapshot = player.snapshot();
        if snapshot.loaded && snapshot.position > seconds {
            return snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "playback did not advance: {snapshot:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A real decode that stores progress, then proves a fresh library instance
/// restores it. This is the automated half of the M2 resume acceptance.
#[test]
fn a_real_playback_session_resumes_after_the_library_is_reopened() {
    let directory = TempDir::new().expect("temp dir");
    let media_root = directory.path().join("media");
    fs::create_dir_all(&media_root).expect("media root");
    let clip = generated_clip(&media_root, "Clip.mkv", 2);

    let database = directory.path().join("library.db");
    let library = Library::open(&database).expect("library");
    let source = library.add_source(&media_root, None).expect("source");
    let parsed = parse_filename("Clip.mkv", None);
    let file_id = library
        .ingest_file(Ingest {
            source_id: &source.id,
            relative_path: "Clip.mkv",
            fingerprint: "clip",
            size: fs::metadata(&clip).unwrap().len() as i64,
            modified_ms: 1,
            scan_id: "scan-1",
            parsed: Some(&parsed),
            expected_root: Some(&source.root_path),
        })
        .expect("ingest");

    let player = match Player::start() {
        Ok(player) => player,
        Err(error) => {
            // No libmpv on this host: the storage half above still ran, and this
            // half reports that it could not rather than pretending to pass.
            eprintln!("skipped the decode half: {error}");
            return;
        }
    };

    let opened = PlaybackTracker::open(&library, &profile(&library), &file_id).expect("open");
    let mut tracker = opened.tracker;
    player.load_at(&clip, Some(opened.resume_from_ms as f64 / 1000.0));

    let snapshot = wait_for_position(&player, 0.4);
    tracker
        .record(
            &library,
            snapshot.position_ms(),
            snapshot.duration_ms(),
            false,
        )
        .expect("checkpoint");
    drop(player);
    tracker.close(&library).expect("close");
    drop(library);

    let (position, _) = stored_position(&database, &file_id);
    assert!(
        position >= 400,
        "stored position was {position} ms, expected at least 400"
    );

    // Reopening must offer exactly that position back to the player.
    let library = Library::open(&database).expect("library");
    let opened = PlaybackTracker::open(&library, &profile(&library), &file_id).expect("open");
    assert_eq!(opened.resume_from_ms, position);
}

/// The app's Replay control reopens a finished file on the same player.
///
/// `PlayerState::Ended` is terminal, so "play the finished file again" is a
/// *second load on one `Player` instance*, not a state change. A player that
/// loads twice but only ever decodes once would leave Replay looking broken and
/// this is the test that catches it.
#[test]
fn a_finished_file_plays_again_on_the_same_player() {
    let directory = TempDir::new().expect("temp dir");
    let media_root = directory.path().join("media");
    fs::create_dir_all(&media_root).expect("media root");
    // Long enough that reaching 1s on the second run is not a race with the end.
    let clip = generated_clip(&media_root, "Replay.mkv", 6);

    let player = match Player::start() {
        Ok(player) => player,
        Err(error) => {
            eprintln!("skipped the decode half: {error}");
            return;
        }
    };

    player.load_at(&clip, Some(0.0));
    let first = wait_for_position(&player, 0.4);
    player.stop();

    // The replay: the same file, loaded again, from the beginning.
    player.load_at(&clip, Some(0.0));
    let deadline = Instant::now() + Duration::from_secs(10);
    while player.snapshot().position > 0.5 {
        assert!(
            Instant::now() < deadline,
            "reloading did not reset the playhead: {:?}",
            player.snapshot()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let second = wait_for_position(&player, 1.0);
    assert!(
        second.position > 1.0,
        "the replay stalled at {} s (first run reached {} s)",
        second.position,
        first.position
    );
}
