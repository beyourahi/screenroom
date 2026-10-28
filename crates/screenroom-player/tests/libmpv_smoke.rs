//! Real libmpv smoke test.
//!
//! This is not a mock: it generates a synthetic clip with FFmpeg, loads the
//! device's libmpv at runtime, opens the file and reads the duration back.
//!
//! It proves the *client API boundary* — dynamic loading, core creation, option
//! setting, command argument passing, event waiting and property reads. It does
//! **not** prove a rendered frame: no render context, GPU surface or window is
//! involved, so it cannot stand in for the desktop presentation gate.

use std::path::Path;
use std::process::Command;

use screenroom_player::{LibMpv, Mpv, capability, event};

fn generate_clip(path: &Path) -> bool {
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=2:size=320x240:rate=10",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(path)
        .status();
    matches!(status, Ok(status) if status.success())
}

#[test]
fn libmpv_opens_a_real_file_and_reports_its_duration() {
    let capability = capability();
    if !capability.available {
        // Report, never pretend: hosts without libmpv cannot produce this receipt.
        eprintln!("SKIPPED: {}", capability.reason);
        return;
    }
    eprintln!(
        "libmpv {} at {}",
        capability.api_version.as_deref().unwrap_or("?"),
        capability.library_path.as_deref().unwrap_or("?")
    );

    let dir = tempfile::tempdir().expect("temp dir");
    let clip = dir.path().join("synthetic.mp4");
    assert!(
        generate_clip(&clip),
        "FFmpeg is required to generate the synthetic client-API clip"
    );

    let library = LibMpv::load().expect("libmpv loads");
    let mut player = Mpv::new(&library).expect("mpv core initialises");
    // Decode without a video output: this exercises demux and decode, not display.
    player.set_option("vo", "null").expect("vo=null");
    player.set_option("ao", "null").expect("ao=null");
    player.set_option("pause", "yes").expect("start paused");

    player
        .command(&["loadfile", &clip.to_string_lossy()])
        .expect("loadfile accepts the path");

    let mut loaded = false;
    for _ in 0..50 {
        match player.wait_event(0.1) {
            event::FILE_LOADED => {
                loaded = true;
                break;
            }
            event::END_FILE | event::SHUTDOWN => break,
            _ => {}
        }
    }
    assert!(loaded, "libmpv reported FILE_LOADED for the synthetic clip");

    let duration = player.get_f64("duration").expect("duration property");
    assert!(
        (1.5..2.5).contains(&duration),
        "expected a two second clip, got {duration}"
    );
}

#[test]
fn a_missing_library_name_is_reported_not_raised() {
    let result = LibMpv::load_path("/nonexistent/libmpv.dylib");
    let error = result.expect_err("a missing path cannot load");
    assert!(error.to_string().contains("not available"));
}
