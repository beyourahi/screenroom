//! Real extraction: ffmpeg, a real file, and the bytes that come back.

use std::process::Command;

use screenroom_desktop::artwork::{extract_poster, sample_position_ms};

/// Builds a short colour test clip, so the frame has real picture in it.
fn clip(directory: &std::path::Path) -> std::path::PathBuf {
    let path = directory.join("clip.mp4");
    let status = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=320x240:rate=25",
            "-t",
            "3",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&path)
        .status()
        .expect("ffmpeg should be installed");
    assert!(status.success(), "ffmpeg could not build the test clip");
    path
}

#[test]
fn a_poster_is_a_scaled_jpeg_of_the_file() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = clip(directory.path());

    let poster = extract_poster(&path, sample_position_ms(3_000)).expect("poster");
    assert!(
        poster.bytes.starts_with(&[0xFF, 0xD8]),
        "a JPEG was expected"
    );
    assert_eq!(poster.width, 400, "the poster is scaled to card width");
    // 320x240 scaled to 400 wide is 400x300, and the height is even.
    assert_eq!(poster.height, 300);
    assert!(
        poster.bytes.len() < 768 * 1024,
        "a poster must stay inside the size cap"
    );
}

#[test]
fn a_file_that_is_not_video_reports_a_reason_instead_of_hanging() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("notes.mkv");
    std::fs::write(&path, b"this is not a video").expect("write");

    let error = extract_poster(&path, 1_000).expect_err("no frame");
    assert!(!error.is_empty(), "a failure must say why");
}

#[test]
fn a_position_past_the_end_falls_back_to_the_first_frame() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = clip(directory.path());

    // A stored duration that is wrong is a real case: the seek finds nothing, and
    // the poster is taken from the opening instead of failing.
    let poster = extract_poster(&path, 10_000_000).expect("poster");
    assert_eq!(poster.sampled_at_ms, 0);
    assert!(poster.bytes.starts_with(&[0xFF, 0xD8]));
}
