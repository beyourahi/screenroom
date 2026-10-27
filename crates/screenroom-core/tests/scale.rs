//! Browsing and search at a library size a real collection reaches.
//!
//! The bounds here are deliberately loose — an order of magnitude above the
//! measured debug figures — because this is not a micro-benchmark. What it
//! catches is the class of mistake that makes a large library unusable: a query
//! that loads every row and pages in memory, an unbounded `OFFSET` that walks the
//! table, a search that scans per row, or a count that runs once per result. Those
//! failures miss a loose bound by seconds, not by percent.
//!
//! `cargo run -p screenroom-core --example library_bench` prints the actual
//! numbers, including a release build's, which is what belongs in the ledger.

use std::time::{Duration, Instant};

use screenroom_core::{Ingest, Library, LibraryQuery, Sort, parse_filename};
use tempfile::TempDir;

/// Entries in the benchmark library: a large personal collection.
const ITEMS: usize = 10_000;

/// Page size used by the desktop grid.
const PAGE: i64 = 24;

/// Ceiling for one paged query or count, generous enough for a debug build on a
/// loaded machine and still far below a full scan of ten thousand rows.
const QUERY_BUDGET: Duration = Duration::from_millis(1_500);

/// Ceiling for the whole ingest of ten thousand entries.
const INGEST_BUDGET: Duration = Duration::from_secs(60);

/// Builds a library of `ITEMS` entries with distinct titles and years.
fn large_library(directory: &TempDir) -> Library {
    let library = Library::open_in_memory().expect("library");
    let source = library.add_source(directory.path(), None).expect("source");
    for index in 0..ITEMS {
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
    library
}

/// Runs one query and fails with the measurement if it exceeded the budget.
fn within_budget(label: &str, budget: Duration, work: impl FnOnce() -> i64) -> Duration {
    let start = Instant::now();
    let rows = work();
    let elapsed = start.elapsed();
    assert!(
        elapsed <= budget,
        "{label} took {elapsed:?} for {rows} row(s), over the {budget:?} budget"
    );
    elapsed
}

#[test]
fn a_ten_thousand_item_library_pages_searches_and_counts_within_budget() {
    let directory = TempDir::new().expect("temp dir");
    let start = Instant::now();
    let library = large_library(&directory);
    let ingest = start.elapsed();
    assert!(
        ingest <= INGEST_BUDGET,
        "ingesting {ITEMS} entries took {ingest:?}, over the {INGEST_BUDGET:?} budget"
    );
    println!("ingest {ITEMS} entries: {ingest:?}");

    // Every page is bounded: the query asks for one page and gets one page.
    let first_page = LibraryQuery {
        sort: Sort::Title,
        ..LibraryQuery::page(PAGE)
    };
    assert_eq!(
        library.items(&first_page).expect("first page").len(),
        PAGE as usize
    );
    let first = within_budget("first page by title", QUERY_BUDGET, || {
        library.items(&first_page).expect("first page").len() as i64
    });
    println!("first page: {first:?}");

    // The last page is the worst case for `OFFSET`: the database has to walk
    // past every earlier row to reach it.
    let last_page = LibraryQuery {
        offset: ITEMS as i64 - PAGE,
        sort: Sort::Title,
        ..LibraryQuery::page(PAGE)
    };
    let last = library.items(&last_page).expect("last page");
    assert_eq!(last.len(), PAGE as usize, "the final page is a full page");
    let last_elapsed = within_budget("last page by title", QUERY_BUDGET, || {
        library.items(&last_page).expect("last page").len() as i64
    });
    println!("last page: {last_elapsed:?}");

    for sort in [Sort::Title, Sort::Added, Sort::Year, Sort::Progress] {
        let query = LibraryQuery {
            sort,
            ..LibraryQuery::page(PAGE)
        };
        let elapsed = within_budget(&format!("{sort:?} ordering"), QUERY_BUDGET, || {
            library.items(&query).expect("page").len() as i64
        });
        println!("{sort:?}: {elapsed:?}");
    }

    // A literal search must find its match and stay bounded.
    let needle = format!("{:05}", ITEMS - 1);
    let search = LibraryQuery {
        text: needle.clone(),
        ..LibraryQuery::page(PAGE)
    };
    let found = library.items(&search).expect("search");
    assert_eq!(
        found.len(),
        1,
        "searching for an exact title finds exactly that entry"
    );
    let search_elapsed = within_budget("literal search", QUERY_BUDGET, || {
        library.items(&search).expect("search").len() as i64
    });
    println!("literal search: {search_elapsed:?}");

    // A search that matches nothing must still be bounded and must report zero
    // rather than falling back to an unfiltered page.
    let miss = LibraryQuery {
        text: "no-such-title-in-this-library".to_string(),
        ..LibraryQuery::page(PAGE)
    };
    assert!(library.items(&miss).expect("miss").is_empty());
    assert_eq!(library.count(&miss).expect("count"), 0);
    within_budget("empty search", QUERY_BUDGET, || {
        library.count(&search).expect("count")
    });

    let counted = library
        .count(&LibraryQuery::page(PAGE))
        .expect("count everything");
    assert_eq!(counted, ITEMS as i64);
    let count_elapsed = within_budget("count all", QUERY_BUDGET, || {
        library.count(&LibraryQuery::page(PAGE)).expect("count")
    });
    println!("count all: {count_elapsed:?}");
}

#[test]
fn a_page_never_returns_more_than_it_was_asked_for() {
    let directory = TempDir::new().expect("temp dir");
    let library = large_library(&directory);

    // A page larger than the contract allows is refused outright rather than
    // silently truncated, so a client cannot think it received everything.
    let huge = LibraryQuery {
        limit: 100_000,
        ..LibraryQuery::page(1)
    };
    let error = library.items(&huge).unwrap_err();
    assert_eq!(error.code(), "invalid_page");

    // Paging walks the whole library without repeating or skipping an entry. The
    // walk uses the largest page the contract allows so the test spends its time
    // on the invariant rather than on four hundred round trips.
    const WALK_PAGE: i64 = 500;
    let mut seen = std::collections::HashSet::new();
    let mut offset = 0;
    while offset < ITEMS as i64 {
        let page = LibraryQuery {
            offset,
            sort: Sort::Title,
            ..LibraryQuery::page(WALK_PAGE)
        };
        let items = library.items(&page).expect("page");
        if items.is_empty() {
            break;
        }
        for item in &items {
            assert!(
                seen.insert(item.id.clone()),
                "{} appeared on two pages",
                item.title
            );
        }
        offset += items.len() as i64;
    }
    assert_eq!(
        seen.len(),
        ITEMS,
        "paging must reach every entry exactly once"
    );
}
