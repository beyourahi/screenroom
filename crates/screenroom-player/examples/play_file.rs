//! Plays real media files offscreen and reports what the player actually did.
//!
//! ```bash
//! cargo run --release -p screenroom-player --example play_file -- "/path/to/file.mkv"
//! cargo run --release -p screenroom-player --example play_file -- file1.mkv file2.mp4 --seconds 4
//! ```
//!
//! This is the real-media counterpart to `render_bench`, which synthesises its own
//! fixture: it takes files you point it at, opens each one through the same worker
//! the window uses, and prints evidence rather than reassurance.
//!
//! For every file it reports:
//!
//! * **open** — time from `load` to the first non-blank decoded frame, or the
//!   player's own failure code and message when it could not open.
//! * **tracks** — every stream libmpv reported, including external subtitle files
//!   it picked up itself, with the selected and external flags.
//! * **audio** — how far the audio device clock advanced while playing. A file that
//!   merely initialised an output does not advance it.
//! * **delivery** — distinct frames per second and the worst gap between them.
//! * **seek** — round trip from the command to the playhead arriving.
//! * **subtitles** — what `sub-scale`/`sub-pos` read back after being set.
//!
//! What it does **not** claim: no window is opened, no GPU surface is used and
//! nothing is heard. A pass here means the file decodes, has an audio clock and
//! accepts track/subtitle commands — not that the window presented it or that the
//! sound was right. Those stay with the owner in `docs/TEST_PLAN.md`.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use screenroom_player::{Frame, Player, TrackKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let seconds: f64 = match arguments
        .iter()
        .position(|argument| argument == "--seconds")
    {
        Some(index) if index + 1 < arguments.len() => {
            let value = arguments.remove(index + 1);
            arguments.remove(index);
            value.parse().unwrap_or(6.0)
        }
        _ => 6.0,
    };
    // Optional: write the first delivered frame as a PNG so the picture itself can
    // be inspected — aspect handling, black bars and subtitle burn-in are not
    // visible in the numbers. Written outside the media root, like every artefact.
    let dump_dir = match arguments
        .iter()
        .position(|argument| argument == "--dump-frame")
    {
        Some(index) if index + 1 < arguments.len() => {
            let value = std::fs::canonicalize(arguments.remove(index + 1)).ok();
            arguments.remove(index);
            value
        }
        _ => None,
    };
    if arguments.is_empty() {
        eprintln!("usage: play_file <file>... [--seconds N]");
        std::process::exit(2);
    }

    let capability = screenroom_player::capability();
    println!(
        "libmpv: {} via {} ({:?} bridge)",
        capability.api_version.as_deref().unwrap_or("-"),
        capability.library_path.as_deref().unwrap_or("-"),
        screenroom_player::preferred_bridge()
    );
    if !capability.available {
        println!("libmpv is unavailable: {}", capability.reason);
        return Ok(());
    }

    for path in arguments {
        println!();
        println!("=== {path}");
        // One worker per file: the lifecycle is then the one the window uses —
        // start, load, play, stop — with nothing carried between files.
        let player = Player::start()?;
        // Muted on purpose: this runs on the owner's machine in the background,
        // and the audio clock still advances while muted — which is the evidence
        // that matters. Nothing is claimed about what the sound was like.
        player.set_muted(true);
        run_one(&player, Path::new(&path), seconds, dump_dir.as_deref());
        player.stop();
    }
    Ok(())
}

/// The height of the black bars at the top and bottom of a frame.
///
/// libmpv scales a video into the render target without stretching it, so an
/// anamorphic or scope picture arrives letterboxed. Measuring the bars is the
/// only way to tell that from a picture the renderer stretched to fill.
fn black_bars(frame: &Frame) -> (u32, u32) {
    let row_is_black = |y: u32| {
        (0..12).all(|index| {
            let x = frame.width.saturating_sub(1) * index / 11;
            frame
                .pixel(x, y)
                .is_none_or(|pixel| pixel[0] < 12 && pixel[1] < 12 && pixel[2] < 12)
        })
    };
    let mut top = 0;
    while top < frame.height && row_is_black(top) {
        top += 1;
    }
    let mut bottom = 0;
    while bottom + top < frame.height && row_is_black(frame.height - 1 - bottom) {
        bottom += 1;
    }
    (top, bottom)
}

/// Writes the current frame as a PNG through FFmpeg, for looking at rather than
/// reasoning about.
fn dump_frame(
    player: &Player,
    path: &Path,
    directory: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let frame = player.snapshot().frame.ok_or("no frame was available")?;
    if !frame.is_well_formed() {
        return Err("the frame was malformed".into());
    }
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "frame".to_string());
    let name = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let output = directory.join(format!("{name}.png"));
    let mut child = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
            &format!("{}x{}", frame.width, frame.height),
            "-i",
            "-",
            "-frames:v",
            "1",
        ])
        .arg(&output)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // libmpv's `rgb0` target documents the alpha byte as undefined, and it comes
    // back as zero: a PNG written straight from it is transparent and looks blank.
    // Force opacity, the same way the presenter does, so the file shows the picture.
    let mut rgba = frame.rgba.as_ref().clone();
    for alpha in rgba.iter_mut().skip(3).step_by(4) {
        *alpha = 255;
    }
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&rgba)?;
    }
    let finished = child.wait_with_output()?;
    if !finished.status.success() {
        return Err(String::from_utf8_lossy(&finished.stderr)
            .trim()
            .to_string()
            .into());
    }
    Ok(output)
}

fn run_one(player: &Player, path: &Path, seconds: f64, dump_dir: Option<&Path>) {
    let started = Instant::now();
    player.load_at(path, None);

    let mut opened = None;
    let mut deadline = started + Duration::from_secs(20);
    while Instant::now() < deadline {
        let snapshot = player.snapshot();
        if let Some(failure) = snapshot.failure.as_ref() {
            println!("  open:      FAILED {} — {}", failure.code, failure.message);
            return;
        }
        if snapshot.loaded
            && let Some(frame) = snapshot.frame.as_ref()
            && !frame.is_blank()
        {
            opened = Some((
                started.elapsed(),
                frame.generation,
                frame.width,
                frame.height,
            ));
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let Some((open_time, first_generation, width, height)) = opened else {
        println!("  open:      NO FRAME within 20s");
        return;
    };
    println!("  open:      {open_time:.2?}, first frame {width}x{height}");
    if let Some(frame) = player.snapshot().frame.as_ref() {
        let (top, bottom) = black_bars(frame);
        println!(
            "  picture:   {width}x{height}, black bars {top}px top / {bottom}px bottom{}",
            if top + bottom >= height {
                " (the frame is black — bars are not meaningful here)"
            } else {
                ""
            }
        );
    }

    if let Some(directory) = dump_dir {
        match dump_frame(player, path, directory) {
            Ok(written) => println!("  frame:     {}", written.display()),
            Err(error) => println!("  frame:     could not be written: {error}"),
        }
    }

    let snapshot = player.snapshot();
    println!("  duration:  {:.2}s", snapshot.duration);
    for track in &snapshot.tracks {
        println!(
            "  track:     {:<8} id={:<4} {:<28} selected={} external={}",
            track.kind.as_str(),
            track.id,
            track.label(),
            track.selected,
            track.external
        );
    }

    // Audio is proved by the output device's own clock, not by a track existing.
    // The clock is absent until the output starts, so wait for it rather than
    // reading it once and calling a quiet start an audio-less file.
    let has_audio_track = snapshot
        .tracks
        .iter()
        .any(|track| track.kind == TrackKind::Audio);
    let audio_wait = Instant::now();
    let mut audio_start = player.snapshot().audio_pts;
    while audio_start.is_none() && audio_wait.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(20));
        audio_start = player.snapshot().audio_pts;
    }
    let delivery_started = Instant::now();
    let mut seen: HashSet<u64> = HashSet::new();
    seen.insert(first_generation);
    let mut last_change = Instant::now();
    let mut worst_gap = Duration::ZERO;
    let mut had_frame = true;

    while delivery_started.elapsed() < Duration::from_secs_f64(seconds) {
        let snapshot = player.snapshot();
        if snapshot.failure.is_some() || snapshot.ended {
            break;
        }
        if let Some(frame) = snapshot.frame.as_ref()
            && seen.insert(frame.generation)
        {
            if had_frame {
                worst_gap = worst_gap.max(last_change.elapsed());
            }
            had_frame = true;
            last_change = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let elapsed = delivery_started.elapsed();
    let delivered = seen.len().saturating_sub(1);
    println!(
        "  delivery:  {delivered} frames in {:.2?} ({:.1} fps), worst gap {worst_gap:.2?}",
        elapsed,
        delivered as f64 / elapsed.as_secs_f64().max(0.001)
    );

    let snapshot = player.snapshot();
    match (audio_start, snapshot.audio_pts) {
        (Some(before), Some(after)) => println!(
            "  audio:     clock advanced {:.2}s in {:.2?} ({})",
            after - before,
            elapsed,
            if after - before > elapsed.as_secs_f64() * 0.5 {
                "playing"
            } else {
                "STALLED"
            }
        ),
        (None, _) if !has_audio_track => println!("  audio:     no audio track"),
        (None, _) => println!("  audio:     no audio clock reported within 3s"),
        (_, None) => println!("  audio:     clock disappeared"),
    }
    println!(
        "  sync:      avsync {:?}, mistimed frames {:?}",
        snapshot.avsync, snapshot.mistimed_frames
    );

    // Seek round trip: issue it, wait for the playhead to arrive near the target.
    let duration = snapshot.duration;
    if duration > 10.0 {
        let target = duration * 0.5;
        let before = player.snapshot().position;
        let issued = Instant::now();
        player.seek(target);
        deadline = issued + Duration::from_secs(10);
        let mut arrived = None;
        while Instant::now() < deadline {
            let position = player.snapshot().position;
            if (position - target).abs() < 1.0 && (before - position).abs() > 0.5 {
                arrived = Some(issued.elapsed());
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        match arrived {
            Some(latency) => {
                println!("  seek:      {before:.1}s → {target:.1}s in {latency:.2?}");
            }
            None => println!("  seek:      playhead never arrived at {target:.1}s"),
        }
    }

    // Subtitle style: what the player read back, not what was asked for.
    player.set_subtitle_style(1.25, 88.0);
    std::thread::sleep(Duration::from_millis(200));
    let snapshot = player.snapshot();
    println!(
        "  subtitles: scale {:?}, position {:?} (asked 1.25 / 88)",
        snapshot.subtitle_scale, snapshot.subtitle_position
    );

    // Track switching: pick the last audio track and confirm the player agrees.
    let audio: Vec<_> = snapshot
        .tracks
        .iter()
        .filter(|track| track.kind == TrackKind::Audio)
        .collect();
    if audio.len() > 1 {
        let target = audio[audio.len() - 1].id;
        player.select_track(TrackKind::Audio, target);
        std::thread::sleep(Duration::from_millis(400));
        let selected = player
            .snapshot()
            .tracks
            .iter()
            .find(|track| track.kind == TrackKind::Audio && track.selected)
            .map(|track| track.id);
        println!(
            "  switch:    asked audio id {target}, player reports {selected:?} selected{}",
            match selected {
                Some(id) if id == target => "",
                _ => "  MISMATCH",
            }
        );
    }
}
