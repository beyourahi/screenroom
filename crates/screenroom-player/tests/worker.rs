//! Real playback through the public player API.
//!
//! These tests need libmpv and FFmpeg on the host. When libmpv is absent they
//! print the reason and return instead of pretending to pass; the receipt only
//! counts where there is a player to exercise.

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use screenroom_player::{Player, PlayerSnapshot, PlayerState};
use tempfile::TempDir;

/// Builds a two-second clip with a moving pattern and a tone.
fn clip(directory: &TempDir) -> PathBuf {
    let path = directory.path().join("clip.mp4");
    let status = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-y",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x240:rate=25:duration=2",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(&path)
        .status()
        .expect("ffmpeg should be installed");
    assert!(status.success(), "ffmpeg could not build the test clip");
    path
}

/// Starts a player, or explains why this host cannot run the test.
fn player() -> Option<Player> {
    match Player::start() {
        Ok(player) => Some(player),
        Err(error) => {
            eprintln!("skipped: {error}");
            None
        }
    }
}

fn wait_for(
    player: &Player,
    timeout: Duration,
    mut done: impl FnMut(&PlayerSnapshot) -> bool,
) -> PlayerSnapshot {
    let deadline = Instant::now() + timeout;
    loop {
        let snapshot = player.snapshot();
        if done(&snapshot) {
            return snapshot;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for the player; last snapshot: {snapshot:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn plays_a_real_file_and_presents_frames() {
    let directory = TempDir::new().expect("temp dir");
    let path = clip(&directory);
    let Some(player) = player() else { return };

    player.load(&path);

    let loaded = wait_for(&player, Duration::from_secs(20), |snapshot| {
        snapshot.loaded && snapshot.frame.is_some()
    });
    assert!(
        matches!(loaded.state, PlayerState::Playing | PlayerState::Paused),
        "unexpected state {:?}",
        loaded.state
    );
    assert!(
        (loaded.duration - 2.0).abs() < 0.3,
        "duration was {}",
        loaded.duration
    );

    let frame = loaded.frame.expect("a frame");
    assert!(frame.is_well_formed(), "frame buffer is malformed");
    assert_eq!((frame.width, frame.height), (320, 240));
    assert!(!frame.is_blank(), "the presented frame was empty");
    // testsrc2 is colourized, so at least one pixel must carry colour.
    let coloured = (0..frame.height)
        .step_by(17)
        .flat_map(|y| (0..frame.width).step_by(17).map(move |x| (x, y)))
        .filter_map(|(x, y)| frame.pixel(x, y))
        .any(|[r, g, b, a]| a == 255 && (r > 8 || g > 8 || b > 8));
    assert!(coloured, "every sampled pixel was black");

    let advanced = wait_for(&player, Duration::from_secs(10), |snapshot| {
        snapshot.position > 0.3
    });
    assert!(advanced.position > 0.3);
}

#[test]
fn pause_holds_the_playhead_and_resume_continues() {
    let directory = TempDir::new().expect("temp dir");
    let path = clip(&directory);
    let Some(player) = player() else { return };

    player.load(&path);
    wait_for(&player, Duration::from_secs(20), |snapshot| {
        snapshot.position > 0.3
    });

    player.set_paused(true);
    let paused = wait_for(&player, Duration::from_secs(5), |snapshot| {
        snapshot.state == PlayerState::Paused
    });
    std::thread::sleep(Duration::from_millis(300));
    let later = player.snapshot();
    assert!(
        (later.position - paused.position).abs() < 0.2,
        "position moved from {} to {} while paused",
        paused.position,
        later.position
    );

    player.set_paused(false);
    wait_for(&player, Duration::from_secs(5), |snapshot| {
        snapshot.state == PlayerState::Playing
    });
}

#[test]
fn seek_moves_the_playhead() {
    let directory = TempDir::new().expect("temp dir");
    let path = clip(&directory);
    let Some(player) = player() else { return };

    player.load(&path);
    wait_for(&player, Duration::from_secs(20), |snapshot| snapshot.loaded);

    player.seek(1.4);
    let sought = wait_for(&player, Duration::from_secs(10), |snapshot| {
        snapshot.position >= 1.2
    });
    assert!(sought.position >= 1.2, "position was {}", sought.position);
}

#[test]
fn resume_position_is_honoured_when_loading() {
    let directory = TempDir::new().expect("temp dir");
    let path = clip(&directory);
    let Some(player) = player() else { return };

    player.load_at(&path, Some(1.2));
    let resumed = wait_for(&player, Duration::from_secs(20), |snapshot| {
        snapshot.loaded && snapshot.position > 0.9
    });
    assert!(
        resumed.position > 0.9,
        "resume did not take effect: {}",
        resumed.position
    );
}

#[test]
fn a_missing_file_is_reported_honestly() {
    let Some(player) = player() else { return };
    player.load("/definitely/not/here.mkv");

    let failed = wait_for(&player, Duration::from_secs(5), |snapshot| {
        snapshot.state == PlayerState::Failed
    });
    let failure = failed.failure.expect("a failure reason");
    assert_eq!(failure.code, "file_missing");
    assert!(failure.message.contains("no longer on disk"));
}
