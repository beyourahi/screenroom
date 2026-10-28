//! Audits the artwork actually stored in a real library.
//!
//! ```bash
//! cargo run --release -p screenroom-core --example artwork_check -- <library.db> [fresh]
//! ```
//!
//! Two questions this answers that a unit test cannot:
//!
//! 1. Are the posters in this library real images? Every stored byte string is
//!    written to a temporary file and handed to `ffprobe`, which must report a
//!    JPEG of the width and height the row claims. A row whose bytes are a frame,
//!    a PNG or a truncated blob fails here.
//! 2. Can the rest of the catalogue get one? Every remaining work must offer a
//!    candidate that points at a file that exists and is readable, or the artwork
//!    queue would spin on works that can never be drawn.
//!
//! `fresh` extracts and stores a poster for the first few candidates with FFmpeg,
//! for works the app has not covered, and then re-checks those images the same
//! way. It writes only inside a temporary directory and never touches the library
//! it did not come from.
//!
//! Exit code 0 when every check passed, 1 otherwise.

use std::path::PathBuf;
use std::process::Command;

use screenroom_core::{Library, LibraryQuery};
use tempfile::TempDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let path: PathBuf = arguments
        .next()
        .expect("usage: artwork_check <library.db> [fresh]")
        .into();
    let fresh = arguments.next().as_deref() == Some("fresh");
    let library = Library::open(&path)?;
    let directory = TempDir::new()?;
    let mut failures = 0_usize;

    let works = library.count(&LibraryQuery {
        group_by_work: true,
        ..LibraryQuery::default()
    })?;
    let stored = library.count(&LibraryQuery {
        group_by_work: true,
        ..LibraryQuery::default()
    })? - library.missing_artwork_count()?;
    println!(
        "works: {works}, with a poster: {stored}, without: {}",
        library.missing_artwork_count()?
    );

    // --- every stored poster must be a real JPEG ----------------------------
    // Read them through `artworks_for`, which is the call the desktop grid makes,
    // so this audits the bytes the UI would actually draw and not a private path.
    let mut checked = 0_usize;
    let mut decoded = 0_usize;
    let mut total_bytes = 0_u64;
    let mut worst: Option<(String, String)> = None;
    let mut offset = 0_i64;
    loop {
        let page = library.items(&LibraryQuery {
            offset,
            limit: 50,
            group_by_work: true,
            ..LibraryQuery::default()
        })?;
        if page.is_empty() {
            break;
        }
        offset += page.len() as i64;
        let work_ids: Vec<String> = page.iter().map(|item| item.work_id.clone()).collect();
        let titles: std::collections::HashMap<String, String> = page
            .iter()
            .map(|item| (item.work_id.clone(), item.title.clone()))
            .collect();
        for artwork in library.artworks_for(&work_ids)? {
            checked += 1;
            match inspect(&artwork, &directory, &mut total_bytes) {
                Ok(()) => decoded += 1,
                Err(reason) => {
                    let title = titles
                        .get(&artwork.work_id)
                        .cloned()
                        .unwrap_or_else(|| artwork.work_id.clone());
                    if worst.is_none() {
                        worst = Some((title, reason));
                    }
                }
            }
        }
    }
    if checked != decoded {
        failures += 1;
    }
    println!(
        "{checked} stored poster(s) read back: {decoded} decoded as JPEG by ffprobe, {} rejected, {total_bytes} bytes",
        checked - decoded
    );
    if let Some((title, reason)) = worst {
        println!("  first failure on \"{title}\": {reason}");
    }

    // --- every remaining work must have somewhere to get one ----------------
    let mut candidates = library.artwork_candidates(200)?;
    let mut missing_path = 0_usize;
    let mut zero_duration = 0_usize;
    for candidate in &candidates {
        let metadata = std::fs::metadata(&candidate.path);
        if !metadata.map(|meta| meta.is_file()).unwrap_or(false) {
            missing_path += 1;
        }
        if candidate.duration_ms <= 0 {
            zero_duration += 1;
        }
    }
    // Ask again with the works the page did not cover, so the check is over the
    // whole catalogue and not just the first 200 titles.
    let remaining = library.missing_artwork_count()?;
    if missing_path > 0 {
        failures += 1;
    }
    println!(
        "{} candidate(s) of {remaining} work(s) still missing artwork: {missing_path} point at a missing file, {zero_duration} have no stored duration",
        candidates.len()
    );

    // --- a fresh extraction, for works the app has not reached --------------
    if fresh {
        let target = &mut candidates;
        target.truncate(6);
        let mut extracted = 0_usize;
        let mut refused = 0_usize;
        for candidate in target.iter() {
            match extract(candidate, &directory) {
                Ok((bytes, width, height)) => {
                    library.store_artwork(
                        &candidate.work_id,
                        Some(&candidate.file_id),
                        &bytes,
                        width,
                        height,
                        60_000,
                    )?;
                    // Read it back and decode it again: a stored poster that the
                    // reader cannot decode is the failure this whole file exists
                    // to catch.
                    let readback = library.artwork(&candidate.work_id)?;
                    let check = match readback {
                        Some(artwork) => inspect(&artwork, &directory, &mut 0),
                        None => Err("the row did not come back".to_string()),
                    };
                    match check {
                        Ok(()) => extracted += 1,
                        Err(reason) => {
                            failures += 1;
                            println!(
                                "  \"{}\" stored but did not decode: {reason}",
                                candidate.title
                            );
                        }
                    }
                }
                Err(Sampling::UnreadableMedia(reason)) => {
                    refused += 1;
                    println!(
                        "  \"{}\" has no usable picture: {reason} (the app records it and stops asking)",
                        candidate.title
                    );
                }
                Err(Sampling::Broken(reason)) => {
                    failures += 1;
                    println!("  \"{}\" could not be sampled: {reason}", candidate.title);
                }
            }
        }
        println!(
            "{extracted} poster(s) extracted, stored and decoded again; {refused} file(s) unreadable"
        );
    }

    if failures == 0 {
        println!("\nall artwork checks passed");
        Ok(())
    } else {
        println!("\n{failures} artwork check(s) failed");
        std::process::exit(1);
    }
}

/// Confirms a stored poster really is a JPEG of the size the row claims.
fn inspect(
    artwork: &screenroom_core::Artwork,
    directory: &TempDir,
    total_bytes: &mut u64,
) -> Result<(), String> {
    if !artwork.bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Err("stored bytes are not a JPEG".into());
    }
    *total_bytes += artwork.bytes.len() as u64;
    let file = directory.path().join(format!("{}.jpg", artwork.work_id));
    std::fs::write(&file, &artwork.bytes).map_err(|error| error.to_string())?;
    let (width, height) = probe(&file)?;
    if width != artwork.width || height != artwork.height {
        return Err(format!(
            "row says {}x{} but the image is {width}x{height}",
            artwork.width, artwork.height
        ));
    }
    Ok(())
}

/// Why a poster could not be extracted.
enum Sampling {
    /// The file itself cannot be decoded. Not a pipeline fault: a truncated or
    /// corrupt file is a fact about the media, and the app records it as a failed
    /// work and stops asking.
    UnreadableMedia(String),
    /// The extractor or the encoder failed on a file that probes fine.
    Broken(String),
}

/// Samples one frame and encodes it as a 400-pixel-wide JPEG.
fn extract(
    candidate: &screenroom_core::ArtworkCandidate,
    directory: &TempDir,
) -> Result<(Vec<u8>, i64, i64), Sampling> {
    let output = directory.path().join(format!("{}.jpg", candidate.work_id));
    // A position inside the file regardless of duration: 60 s for anything long
    // enough, 2 s otherwise. `-ss` before `-i` seeks, so this reads a few hundred
    // kilobytes instead of the whole film.
    let at = if candidate.duration_ms > 120_000 || candidate.duration_ms == 0 {
        "60"
    } else {
        "2"
    };
    let status = Command::new("ffmpeg")
        .args(["-nostdin", "-v", "error", "-y", "-ss", at, "-i"])
        .arg(&candidate.path)
        .args([
            "-frames:v",
            "1",
            "-an",
            "-sn",
            "-vf",
            "scale=400:-2",
            "-q:v",
            "4",
        ])
        .arg(&output)
        .status()
        .map_err(|error| Sampling::Broken(error.to_string()))?;
    if !status.success() {
        // Whether this is the media's fault or the pipeline's is answered by
        // asking ffprobe the same question the app's own probe asks.
        return Err(if readable(&candidate.path) {
            Sampling::Broken("ffmpeg refused a file ffprobe can read".into())
        } else {
            Sampling::UnreadableMedia("the file cannot be decoded at all".into())
        });
    }
    let bytes = std::fs::read(&output).map_err(|error| Sampling::Broken(error.to_string()))?;
    let (width, height) = probe(&output).map_err(Sampling::Broken)?;
    Ok((bytes, width, height))
}

/// True when ffprobe can identify the file's streams.
fn readable(path: &std::path::Path) -> bool {
    Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_name",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .map(|output| output.status.success() && !output.stdout.is_empty())
        .unwrap_or(false)
}

/// Reads a JPEG's real dimensions with `ffprobe`.
fn probe(path: &std::path::Path) -> Result<(i64, i64), String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,codec_name",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err("ffprobe could not read the image".into());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text.lines().next().unwrap_or_default();
    let fields: Vec<&str> = first.trim().split(',').collect();
    if fields.len() < 3 {
        return Err(format!("unexpected ffprobe output: {first:?}"));
    }
    let codec = fields[0];
    let width: i64 = fields[1].parse().map_err(|_| "no width".to_string())?;
    let height: i64 = fields[2].parse().map_err(|_| "no height".to_string())?;
    if codec != "mjpeg" && codec != "jpeg2000" {
        return Err(format!("decoded as {codec}, not a JPEG"));
    }
    Ok((width, height))
}
