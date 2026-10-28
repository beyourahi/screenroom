//! Runs the application's own poster path over a library's missing posters.
//!
//! `cargo run --release -p screenroom-desktop --example poster_check -- LIB.DB [COUNT]`
//!
//! Writes posters and durations into `LIB.DB`, so give it a copy. Reads the media
//! only; prints no titles or paths, just what each attempt produced.

use std::sync::Arc;
use std::time::Instant;

use screenroom_core::Library;
use screenroom_desktop::artwork::best_poster;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: poster_check LIB.DB [COUNT]");
        std::process::exit(2);
    };
    let count: i64 = args.next().and_then(|n| n.parse().ok()).unwrap_or(10);
    let library = Arc::new(Library::open(&path).expect("open library"));
    let candidates = library.artwork_candidates(count).expect("candidates");
    let (mut covers, mut frames, mut failed) = (0, 0, 0);
    for (index, candidate) in candidates.iter().enumerate() {
        let started = Instant::now();
        match best_poster(&library, candidate) {
            Ok(poster) => {
                library
                    .store_artwork(
                        &candidate.work_id,
                        Some(&candidate.file_id),
                        &poster.bytes,
                        poster.width,
                        poster.height,
                        poster.sampled_at_ms,
                    )
                    .expect("store");
                if poster.cover {
                    covers += 1;
                } else {
                    frames += 1;
                }
                println!(
                    "#{index:<3} {:<5} {}x{} {} bytes, at {} ms, {:.2?}",
                    if poster.cover { "cover" } else { "frame" },
                    poster.width,
                    poster.height,
                    poster.bytes.len(),
                    poster.sampled_at_ms,
                    started.elapsed()
                );
            }
            Err(reason) => {
                failed += 1;
                println!("#{index:<3} FAILED {reason} ({:.2?})", started.elapsed());
            }
        }
    }
    println!("covers={covers} frames={frames} failed={failed}");
}
