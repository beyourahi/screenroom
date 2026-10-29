//! Measures one real media file with one copy-back policy.
//!
//! Example output is written only to Screenroom's local development evidence
//! directory. The media path is opened by libmpv and never modified.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use screenroom_player::{HwdecMode, Player};

const EVIDENCE_DIR: &str =
    "/Users/beyourahi/.local/state/dev-evidence/screenroom-macos-final/codex-hwdec";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 4 {
        return Err(
            "usage: hwdec_measure <no|videotoolbox-copy|auto-copy> <label> <media-path> <seconds>"
                .into(),
        );
    }
    let mode = match args[0].as_str() {
        "no" => HwdecMode::No,
        "videotoolbox-copy" => HwdecMode::VideoToolboxCopy,
        "auto-copy" => HwdecMode::AutoCopy,
        other => return Err(format!("unsupported mode: {other}").into()),
    };
    let label = &args[1];
    let media = PathBuf::from(&args[2]);
    let seconds: u64 = args[3].parse()?;
    if seconds < 20 {
        return Err("sustained playback measurement must run for at least 20 seconds".into());
    }
    if !media.is_absolute() || !media.is_file() {
        return Err("media path must be an existing absolute file".into());
    }
    if !label
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err("label may contain ASCII letters, digits and hyphens only".into());
    }

    std::fs::create_dir_all(EVIDENCE_DIR)?;
    let capability = screenroom_player::capability();
    if !capability.available {
        return Err(format!("libmpv unavailable: {}", capability.reason).into());
    }
    let player = Player::start_with_hwdec(mode)?;
    player.set_muted(true);
    let began = Instant::now();
    player.load(&media);

    let first_frame = wait_for_frame(&player, Duration::from_secs(90))?;
    let first_frame_at = began.elapsed();
    let expected_dimensions = (first_frame.width, first_frame.height);
    let first_frame_generation = first_frame.generation;
    let png = Path::new(EVIDENCE_DIR).join(format!("{label}-{}-sampled-frame.png", args[0]));

    let sampling_started = Instant::now();
    let sampling_duration = Duration::from_secs(seconds);
    let mut last_seen_generation = first_frame_generation;
    let mut delivered = 0_u64;
    let mut last_change = Instant::now();
    let mut worst_gap = Duration::ZERO;
    let mut observed_generations = HashSet::new();
    observed_generations.insert(first_frame_generation);
    let mut cpu = Vec::<f64>::new();
    let mut rss_kb = Vec::<u64>::new();
    let mut next_process_sample = sampling_started + Duration::from_secs(1);

    while sampling_started.elapsed() < sampling_duration {
        let snapshot = player.snapshot();
        if let Some(frame) = snapshot.frame.as_ref()
            && observed_generations.insert(frame.generation)
        {
            let generation_delta = frame.generation.saturating_sub(last_seen_generation);
            delivered = delivered.saturating_add(generation_delta);
            if last_seen_generation != frame.generation {
                worst_gap = worst_gap.max(last_change.elapsed());
                last_change = Instant::now();
                last_seen_generation = frame.generation;
            }
        }
        if snapshot.failure.is_some() {
            return Err(format!("playback failed: {:?}", snapshot.failure).into());
        }
        if Instant::now() >= next_process_sample {
            if let Some((percent, rss)) = sample_process(std::process::id()) {
                cpu.push(percent);
                rss_kb.push(rss);
            }
            next_process_sample += Duration::from_secs(1);
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    let elapsed = sampling_started.elapsed();
    worst_gap = worst_gap.max(last_change.elapsed());
    let final_snapshot = player.snapshot();
    let frame = final_snapshot.frame.as_ref().ok_or("frame disappeared")?;
    let luma = mean_luma(frame);
    write_png(frame, &png)?;
    let frame_non_blank = frame
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .any(|pixel| pixel[0] > 8 || pixel[1] > 8 || pixel[2] > 8);
    let (cpu_mean, cpu_max) = mean_max(&cpu);
    let (rss_mean, rss_max) = mean_max_u64(&rss_kb);
    let hwdec = final_snapshot
        .hardware_decoder
        .as_deref()
        .unwrap_or("unavailable");
    let dimensions = format!("{}x{}", expected_dimensions.0, expected_dimensions.1);
    let fps = delivered as f64 / elapsed.as_secs_f64();
    let worst_gap_ms = worst_gap.as_secs_f64() * 1_000.0;

    println!(
        "label,mode,first_frame_ms,render_dimensions,fps,worst_gap_ms,cpu_mean_percent,cpu_max_percent,rss_mean_kb,rss_max_kb,hwdec_current,frame_non_blank,mean_luma,png"
    );
    println!(
        "{label},{},{:.2},{dimensions},{fps:.3},{worst_gap_ms:.2},{cpu_mean:.2},{cpu_max:.2},{rss_mean:.0},{rss_max},{hwdec},{frame_non_blank},{luma:.4},{}",
        args[0],
        first_frame_at.as_secs_f64() * 1_000.0,
        png.display()
    );
    player.stop();
    Ok(())
}

fn wait_for_frame(
    player: &Player,
    timeout: Duration,
) -> Result<screenroom_player::Frame, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let snapshot = player.snapshot();
        if let Some(failure) = snapshot.failure {
            return Err(format!("playback failed: {failure:?}").into());
        }
        if snapshot.loaded
            && let Some(frame) = snapshot.frame
            && !frame.is_blank()
        {
            return Ok(frame);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Err("no non-blank frame arrived within 90 seconds".into())
}

fn mean_luma(frame: &screenroom_player::Frame) -> f64 {
    let pixels = frame.rgba.as_chunks::<4>().0.iter();
    let sum = pixels
        .map(|pixel| {
            0.2126 * f64::from(pixel[0])
                + 0.7152 * f64::from(pixel[1])
                + 0.0722 * f64::from(pixel[2])
        })
        .sum::<f64>();
    sum / (frame.width as f64 * frame.height as f64 * 255.0)
}

fn write_png(
    frame: &screenroom_player::Frame,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if !output.starts_with(EVIDENCE_DIR) {
        return Err("PNG output escaped the evidence directory".into());
    }
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
            "pipe:0",
            "-frames:v",
            "1",
        ])
        .arg(output)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut rgba = frame.rgba.as_ref().clone();
    for alpha in rgba.iter_mut().skip(3).step_by(4) {
        *alpha = 255;
    }
    child
        .stdin
        .take()
        .ok_or("ffmpeg stdin unavailable")?
        .write_all(&rgba)?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr)
            .trim()
            .to_string()
            .into());
    }
    Ok(())
}

fn sample_process(pid: u32) -> Option<(f64, u64)> {
    let output = Command::new("ps")
        .args(["-o", "%cpu=,rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    let mut values = text.split_whitespace();
    Some((values.next()?.parse().ok()?, values.next()?.parse().ok()?))
}

fn mean_max(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    (mean, values.iter().copied().fold(0.0, f64::max))
}

fn mean_max_u64(values: &[u64]) -> (f64, u64) {
    if values.is_empty() {
        return (0.0, 0);
    }
    let mean = values.iter().sum::<u64>() as f64 / values.len() as f64;
    (mean, values.iter().copied().max().unwrap_or(0))
}
