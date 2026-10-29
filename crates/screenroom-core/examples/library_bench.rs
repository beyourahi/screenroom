//! Measures one library at a chosen size, and prints what it measured.
//!
//! ```text
//! cargo run --release -p screenroom-core --example library_bench
//! cargo run --release -p screenroom-core --example library_bench -- 50000
//! ```
//!
//! This exists so the numbers in `docs/IMPLEMENTATION_STATUS.md` are reproducible
//! from a command rather than remembered. It writes nothing outside a temporary
//! directory, needs no media, and touches the network never.
//!
//! It reports the two things that decide whether a large library feels usable: how
//! long the ingest takes, and how long a page, a search and a count take once it is
//! built. A page and its count are timed separately because the Browse screen runs
//! both on every keystroke in the search field.

use std::time::{Duration, Instant};

use screenroom_core::{Ingest, Library, LibraryQuery, Sort, parse_filename};
use tempfile::TempDir;

fn main() {
    let count: usize = std::env::args()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(10_000);
    let directory = TempDir::new().expect("temp dir");
    let library = Library::open_in_memory().expect("library");
    let source = library.add_source(directory.path(), None).expect("source");

    let started = Instant::now();
    for index in 0..count {
        let name = format!("Title {index:05} ({}).mkv", 1950 + index % 70);
        let parsed = parse_filename(&name, None);
        library
            .ingest_file(Ingest {
                source_id: &source.id,
                relative_path: &name,
                fingerprint: &format!("fingerprint-{index}"),
                size: 1_000,
                modified_ms: 1,
                scan_id: "bench",
                parsed: Some(&parsed),
                expected_root: Some(&source.root_path),
            })
            .expect("ingest");
    }
    let ingest = started.elapsed();
    println!(
        "ingest {count} entries: {ingest:?} ({:.0} entries/s)",
        count as f64 / ingest.as_secs_f64()
    );
    println!();

    let mut slowest = Duration::ZERO;
    let mut slowest_label = String::new();
    let mut measure = |label: &str, query: &LibraryQuery| {
        let started = Instant::now();
        let rows = library.items(query).expect("page").len();
        let page = started.elapsed();
        let started = Instant::now();
        let total = library.count(query).expect("count");
        let count_time = started.elapsed();
        println!("{label:<22} {rows:>4} rows {page:>12?}   count {total:>6} {count_time:>12?}");
        if page > slowest {
            slowest = page;
            slowest_label = label.to_string();
        }
    };

    measure(
        "first page",
        &LibraryQuery {
            sort: Sort::Title,
            ..LibraryQuery::page(24)
        },
    );
    // The worst case for `OFFSET`: the database walks past every earlier row.
    measure(
        "last page",
        &LibraryQuery {
            offset: count as i64 - 24,
            sort: Sort::Title,
            ..LibraryQuery::page(24)
        },
    );
    for sort in [Sort::Added, Sort::Year, Sort::Progress] {
        measure(
            &format!("{sort:?} ordering"),
            &LibraryQuery {
                sort,
                ..LibraryQuery::page(24)
            },
        );
    }
    measure(
        "literal search",
        &LibraryQuery {
            text: format!("{:05}", count.saturating_sub(1)),
            ..LibraryQuery::page(24)
        },
    );
    measure(
        "search with no match",
        &LibraryQuery {
            text: "no-such-title".to_string(),
            ..LibraryQuery::page(24)
        },
    );
    measure("count everything", &LibraryQuery::page(24));
    // What the desktop's Browse screen runs: one card per work, for a viewer,
    // with the watched filter applied. Grouping is a window function, so it is
    // timed on its own rather than assumed to cost what a flat page does.
    let viewer = library.profiles().expect("profiles").remove(0).id;
    for (label, offset) in [
        ("grouped first page", 0),
        ("grouped last page", count as i64 - 24),
    ] {
        measure(
            label,
            &LibraryQuery {
                profile_id: Some(viewer.clone()),
                group_by_work: true,
                watched: Some(false),
                offset: offset.max(0),
                sort: Sort::Title,
                ..LibraryQuery::page(24)
            },
        );
    }

    // The Browse and Home screens ask the catalogue for its facets before they
    // draw a single card, on the UI thread, so these are timed separately from
    // paging: a slow facet is a window that does not open.
    let profile = library.profiles().expect("profiles").remove(0);
    let started = Instant::now();
    let genres = library.known_genres().expect("genres");
    let genres_time = started.elapsed();
    println!(
        "{:<22} {:>4} genres {genres_time:>12?}",
        "known genres",
        genres.len()
    );

    let started = Instant::now();
    let watching = library
        .continue_watching(&profile.id, 20)
        .expect("continue watching");
    let watching_time = started.elapsed();
    println!(
        "{:<22} {:>4} rows {watching_time:>12?}",
        "continue watching",
        watching.len()
    );

    println!();
    println!("slowest page: {slowest_label} at {slowest:?}");
}
