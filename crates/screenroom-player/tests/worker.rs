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
    // testsrc2 is colourized, so at least one pixel must carry colour. Only the
    // colour channels are checked: the frame's alpha byte is documented as
    // undefined, and the presenter is what forces opaque output.
    let coloured = (0..frame.height)
        .step_by(17)
        .flat_map(|y| (0..frame.width).step_by(17).map(move |x| (x, y)))
        .filter_map(|(x, y)| frame.pixel(x, y))
        .any(|[r, g, b, _]| r > 8 || g > 8 || b > 8);
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

/// Builds a longer, full-resolution clip for the rate measurement.
///
/// The audio track is not decoration: the player runs with `video-sync=audio`,
/// so a silent clip takes a different pacing path than real media and would
/// misreport the bridge's rate.
fn timing_clip(directory: &TempDir) -> PathBuf {
    let path = directory.path().join("timing.mp4");
    let status = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-y",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1920x1080:rate=30:duration=8",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=8",
            "-ac",
            "2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-preset",
            "veryfast",
            "-crf",
            "28",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-shortest",
        ])
        .arg(&path)
        .status()
        .expect("ffmpeg should be installed");
    assert!(status.success(), "ffmpeg could not build the timing clip");
    path
}

/// Measures the frame rate of the software presentation bridge at 1080p.
///
/// This is the production path the desktop app draws from: libmpv's software
/// renderer writes an RGBA buffer and publishes it with a new generation number.
/// Counting those generations over a wall-clock interval is the rate the window
/// can actually be fed. The measured figure is printed, because the desktop
/// frame-rate gate needs a number rather than "it looked smooth".
///
/// The source runs at 30 fps; the floor asserted here is deliberately below that,
/// so the test fails on a real collapse rather than on a busy machine.
#[test]
fn the_software_bridge_sustains_a_watchable_frame_rate() {
    let directory = TempDir::new().expect("temp dir");
    let path = timing_clip(&directory);
    let Some(player) = player() else { return };
    player.load(&path);
    let started = wait_for(&player, Duration::from_secs(30), |snapshot| {
        snapshot.frame.is_some() && snapshot.position > 0.3
    });

    // Sampled at two points rather than polled: the generation number is
    // monotonic per published frame, so the difference is exact and the
    // measurement itself cannot become the bottleneck.
    let first = started
        .frame
        .as_ref()
        .map(|frame| frame.generation)
        .unwrap_or(0);
    let began = Instant::now();
    std::thread::sleep(Duration::from_secs(4));
    let ended = player.snapshot();
    let elapsed = began.elapsed().as_secs_f64();

    let presented = ended
        .frame
        .as_ref()
        .map(|frame| frame.generation)
        .unwrap_or(first)
        .saturating_sub(first);
    let rate = presented as f64 / elapsed.max(0.001);
    // How far the playhead moved over the same interval: below real time means
    // the media is waiting on this pipeline, not that frames were dropped.
    let advance = (ended.position - started.position) / elapsed.max(0.001);
    println!(
        "software bridge: {presented} frames in {elapsed:.2} s = {rate:.1} fps at 1920x1080 \
         (playhead advanced {advance:.2}x real time)"
    );
    assert!(
        rate >= 20.0,
        "the bridge published {rate:.1} fps, too slow to watch"
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
