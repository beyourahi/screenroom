//! Checks a real library through the public API and prints one line per check.
//!
//! ```bash
//! cargo run --release -p screenroom-core --example library_check -- <library.db>
//! ```
//!
//! `tests/` proves the contracts on fixtures it builds itself. This proves the same
//! contracts still hold on a library that came from a real drive: paging walks every
//! entry once, search finds what exists and nothing when it does not, sorts and
//! filters agree with the count, a session produces a resume point, lists filter,
//! and a backup restores with the same numbers.
//!
//! It is safe on a **copy** of a real library but not on the library itself: it
//! writes a favourite, a progress row, a preference and a throwaway profile, then
//! restores a backup into a temporary database. The backup it writes is deleted
//! with the temporary directory and must never be committed — it lists private
//! media paths.
//!
//! Exit code 0 when every check passed, 1 when any failed.

use std::collections::HashSet;
use std::path::PathBuf;

use screenroom_core::{Checkpoint, Library, LibraryQuery, Sort};
use tempfile::TempDir;

/// Prints one check and returns whether it failed.
struct Checks {
    failures: usize,
    passed: usize,
    skipped: usize,
}

impl Checks {
    fn new() -> Self {
        Self {
            failures: 0,
            passed: 0,
            skipped: 0,
        }
    }

    /// A check this library cannot answer. Counted separately so a vacuous pass
    /// never reads as evidence.
    fn skip(&mut self, label: &str, why: impl AsRef<str>) {
        self.skipped += 1;
        println!("SKIP {label}: {}", why.as_ref());
    }

    fn check(&mut self, label: &str, ok: bool, detail: impl AsRef<str>) {
        if ok {
            self.passed += 1;
        } else {
            self.failures += 1;
        }
        println!(
            "{} {label}: {}",
            if ok { "PASS" } else { "FAIL" },
            detail.as_ref()
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path: PathBuf = std::env::args()
        .nth(1)
        .expect("usage: library_check <library.db>")
        .into();
    if !path.is_file() {
        eprintln!("{} is not a file", path.display());
        std::process::exit(2);
    }

    let library = Library::open(&path)?;
    let profile = library
        .profiles()?
        .into_iter()
        .next()
        .ok_or("the library has no profile")?;
    let mut checks = Checks::new();

    let total = library.count(&LibraryQuery::default())?;
    checks.check("catalogue has entries", total > 0, format!("{total} rows"));
    if total == 0 {
        println!("\nnothing to check");
        std::process::exit(1);
    }

    // --- paging -------------------------------------------------------------
    // Walk the whole catalogue in the largest page the contract allows and prove
    // every row arrives exactly once. A duplicated or dropped row here is the
    // reason a grid would show a title twice or lose one between pages.
    let page_size = 500_i64;
    let mut seen: HashSet<String> = HashSet::new();
    let mut duplicated = 0_usize;
    let mut offset = 0_i64;
    while offset < total {
        let page = library.items(&LibraryQuery {
            sort: Sort::Title,
            offset,
            limit: page_size,
            ..LibraryQuery::default()
        })?;
        if page.is_empty() {
            break;
        }
        for item in &page {
            if !seen.insert(item.id.clone()) {
                duplicated += 1;
            }
        }
        offset += page.len() as i64;
    }
    checks.check(
        "paging reaches every entry exactly once",
        duplicated == 0 && seen.len() == total as usize,
        format!(
            "{} unique of {total} after {} page(s), {duplicated} repeated",
            seen.len(),
            (offset + page_size - 1) / page_size
        ),
    );

    // An oversized page is refused rather than honoured or silently truncated: a
    // client cannot make the library materialise 100k rows, and it must not look
    // like the answer when it did not.
    let oversized = library.items(&LibraryQuery {
        limit: 100_000,
        ..LibraryQuery::default()
    });
    checks.check(
        "an oversized page is refused",
        oversized.is_err(),
        "invalid_page",
    );

    // --- ordering -----------------------------------------------------------
    for sort in [Sort::Title, Sort::Added, Sort::Year, Sort::Progress] {
        let rows = library.items(&LibraryQuery {
            sort,
            limit: 24,
            ..LibraryQuery::default()
        })?;
        checks.check(
            &format!("{sort:?} ordering"),
            rows.len() == total.min(24) as usize,
            format!("{} rows", rows.len()),
        );
    }

    // Title ordering must actually be sorted, not just non-empty.
    let first_page = library.items(&LibraryQuery {
        sort: Sort::Title,
        limit: 200,
        ..LibraryQuery::default()
    })?;
    let sorted = first_page
        .windows(2)
        .all(|pair| pair[0].title.to_lowercase() <= pair[1].title.to_lowercase());
    checks.check(
        "title ordering is ascending",
        sorted,
        format!("{} rows compared", first_page.len()),
    );

    // --- search -------------------------------------------------------------
    let first = library
        .items(&LibraryQuery {
            sort: Sort::Title,
            limit: 1,
            ..LibraryQuery::default()
        })?
        .remove(0);
    let needle = first.title.clone();
    let hits = library.count(&LibraryQuery {
        text: needle.clone(),
        ..LibraryQuery::default()
    })?;
    checks.check(
        "search finds an existing title",
        hits >= 1,
        format!("{hits} row(s) for a known title"),
    );

    // Search is a substring match, so a title's first word must also find it.
    if let Some(word) = needle.split_whitespace().next() {
        let by_word = library.count(&LibraryQuery {
            text: word.to_string(),
            ..LibraryQuery::default()
        })?;
        checks.check(
            "search matches on a word",
            by_word >= hits,
            format!("{by_word} row(s) for \"{word}\""),
        );
    }

    // LIKE metacharacters must be literal: `%` finds nothing unless a title has one.
    let wildcard = library.count(&LibraryQuery {
        text: "%".into(),
        ..LibraryQuery::default()
    })?;
    let with_percent = library
        .items(&LibraryQuery {
            limit: page_size,
            ..LibraryQuery::default()
        })?
        .iter()
        .filter(|item| item.title.contains('%'))
        .count();
    checks.check(
        "search treats % as a literal",
        with_percent > 0 || wildcard == 0,
        format!("{wildcard} row(s) for \"%\" on the first page"),
    );

    let miss = library.count(&LibraryQuery {
        text: "zzz no such title in this library".into(),
        ..LibraryQuery::default()
    })?;
    checks.check(
        "search for nonsense finds nothing",
        miss == 0,
        format!("{miss} row(s)"),
    );

    // --- filters ------------------------------------------------------------
    let movies = library.count(&LibraryQuery {
        kind: Some("movie".into()),
        ..LibraryQuery::default()
    })?;
    let series = library.count(&LibraryQuery {
        kind: Some("series".into()),
        ..LibraryQuery::default()
    })?;
    let unknown = library.count(&LibraryQuery {
        kind: Some("unknown".into()),
        ..LibraryQuery::default()
    })?;
    checks.check(
        "kind filters partition the catalogue",
        movies + series + unknown == total,
        format!(
            "{movies} films + {series} episodes + {unknown} unknown = {}",
            movies + series + unknown
        ),
    );

    let bad_kind = library.count(&LibraryQuery {
        kind: Some("podcast".into()),
        ..LibraryQuery::default()
    });
    checks.check(
        "an unknown kind is refused",
        bad_kind.is_err(),
        "invalid_filter",
    );

    // Genre filters need genres, and no metadata provider is configured yet, so a
    // scanned library carries none. Say so instead of passing on an empty set.
    let genres = library.known_genres()?;
    if genres.is_empty() {
        checks.skip(
            "genre filter",
            "this library stores no genres; the filter is covered by unit tests",
        );
    } else if let Some(genre) = genres.first() {
        let genre_count = library.count(&LibraryQuery {
            genre: Some(genre.clone()),
            ..LibraryQuery::default()
        })?;
        let filtered = library.items(&LibraryQuery {
            genre: Some(genre.clone()),
            limit: page_size,
            ..LibraryQuery::default()
        })?;
        checks.check(
            "genre filter agrees with its count",
            genre_count > 0 && filtered.iter().all(|item| item.genres.contains(genre)),
            format!("{genre_count} row(s), all carrying the label"),
        );
    }

    let available = library.count(&LibraryQuery {
        available_only: true,
        ..LibraryQuery::default()
    })?;
    checks.check(
        "available-only filter",
        available == total,
        format!("{available} of {total} available"),
    );

    // Seasons are asked for on a real series, not on whatever title ordering put
    // first — that row is a film, and a film has no seasons to prove.
    let a_series = library.items(&LibraryQuery {
        kind: Some("series".into()),
        sort: Sort::Title,
        limit: 1,
        ..LibraryQuery::default()
    })?;
    if let Some(series_item) = a_series.first() {
        let seasons = library.seasons_for_work(&series_item.work_id)?;
        let distinct = seasons.iter().collect::<HashSet<_>>().len() == seasons.len();
        checks.check(
            "seasons are listed per work",
            !seasons.is_empty() && distinct,
            format!("{} season(s) for a series", seasons.len()),
        );
        // The episodes of one season must all belong to that season and work.
        if let Some(season) = seasons.first() {
            let episodes = library.items(&LibraryQuery {
                kind: Some("series".into()),
                work_id: Some(series_item.work_id.clone()),
                season: Some(*season),
                limit: page_size,
                ..LibraryQuery::default()
            })?;
            checks.check(
                "an episode page belongs to one season",
                !episodes.is_empty()
                    && episodes.iter().all(|item| {
                        item.season == Some(*season) && item.work_id == series_item.work_id
                    }),
                format!("{} episode(s) in season {season}", episodes.len()),
            );
        }
    } else {
        checks.check(
            "seasons are listed per work",
            false,
            "no series in the library",
        );
    }

    // --- progress -----------------------------------------------------------
    // A scan stores no durations, so this supplies one for the check only.
    let session = library.start_session(&profile.id, &first.id)?;
    // A session opens at seek generation 0; `begin_seek` is what advances it.
    let saved = library.checkpoint(Checkpoint {
        session: &session,
        position_ms: 120_000,
        duration_ms: 600_000,
        sequence: 1,
        seek_generation: 0,
        ended: false,
    })?;
    let watching = library.continue_watching(&profile.id, 20)?;
    checks.check(
        "a checkpoint becomes a resume point",
        saved && watching.iter().any(|item| item.id == first.id),
        format!("{} item(s) in Continue watching", watching.len()),
    );

    // After a seek, a report still carrying the pre-seek generation is stale: it
    // must be dropped rather than rewinding the playhead the seek moved.
    let after_seek = library.begin_seek(&session)?;
    let stale = library.checkpoint(Checkpoint {
        session: &session,
        position_ms: 1_000,
        duration_ms: 600_000,
        sequence: 2,
        seek_generation: 0,
        ended: false,
    })?;
    let accepted = library.checkpoint(Checkpoint {
        session: &session,
        position_ms: 120_000,
        duration_ms: 600_000,
        sequence: 3,
        seek_generation: after_seek,
        ended: false,
    })?;
    checks.check(
        "the post-seek generation is accepted",
        accepted,
        format!("generation {after_seek}"),
    );
    let position_now = library
        .item(&first.id, Some(&profile.id))?
        .map(|item| item.position_ms)
        .unwrap_or(0);
    checks.check(
        "a stale generation cannot rewind progress",
        !stale && position_now == 120_000,
        format!("position {position_now} ms after the stale report"),
    );

    library.end_session(&session)?;

    // --- lists and recommendations -----------------------------------------
    // Lists are per profile: the query must name the profile whose list is asked
    // for, which is also proof the flag is not global.
    let listed_query = |field: &str| LibraryQuery {
        profile_id: Some(profile.id.clone()),
        favorites_only: field == "favorite",
        watchlist_only: field == "watchlisted",
        ..LibraryQuery::default()
    };
    library.set_favorite(&profile.id, &first.work_id, true)?;
    let favourites = library.count(&listed_query("favorite"))?;
    library.set_watchlisted(&profile.id, &first.work_id, true)?;
    let listed = library.count(&listed_query("watchlisted"))?;
    library.set_favorite(&profile.id, &first.work_id, false)?;
    library.set_watchlisted(&profile.id, &first.work_id, false)?;
    let cleared = library.count(&listed_query("favorite"))?;
    checks.check(
        "favourite and watchlist filters",
        favourites >= 1 && listed >= 1 && cleared == 0,
        format!("{favourites} favourite, {listed} watchlisted, {cleared} after clearing"),
    );

    let suggestions = library.recommendations(&profile.id, 20)?;
    let explained = suggestions.iter().all(|s| !s.reason.is_empty());
    let unique_works = suggestions
        .iter()
        .map(|s| s.item.work_id.clone())
        .collect::<HashSet<_>>()
        .len()
        == suggestions.len();
    checks.check(
        "recommendations are explained and deduplicated",
        explained && unique_works,
        format!(
            "{} suggestion(s), {} distinct, reasons {}",
            suggestions.len(),
            suggestions
                .iter()
                .map(|s| s.item.work_id.clone())
                .collect::<HashSet<_>>()
                .len(),
            if explained { "present" } else { "missing" }
        ),
    );

    // --- preferences --------------------------------------------------------
    let before = library.preferences()?;
    let mut changed = before.clone();
    changed.subtitle_scale_percent = 115;
    changed.autoplay_next = !before.autoplay_next;
    changed.volume_percent = 45;
    library.set_preferences(&changed)?;
    drop(library);
    let reopened = Library::open(&path)?;
    let stored = reopened.preferences()?;
    checks.check(
        "preferences survive a reopen",
        stored == changed,
        format!(
            "scale {}%, autoplay {}, volume {}%",
            stored.subtitle_scale_percent, stored.autoplay_next, stored.volume_percent
        ),
    );
    reopened.set_preferences(&before)?;

    // --- profiles -----------------------------------------------------------
    let added = reopened.add_profile("Contract check")?;
    reopened.rename_profile(&added.id, "Contract checked")?;
    let renamed = reopened
        .profiles()?
        .into_iter()
        .any(|profile| profile.id == added.id && profile.name == "Contract checked");
    reopened.remove_profile(&added.id)?;
    let gone = !reopened
        .profiles()?
        .iter()
        .any(|profile| profile.id == added.id);
    checks.check(
        "profiles add, rename and remove",
        renamed && gone,
        format!("{} profile(s) remain", reopened.profiles()?.len()),
    );

    // --- backup and restore -------------------------------------------------
    let directory = TempDir::new()?;
    let backup = directory.path().join("library.json");
    let written = reopened.backup_to(&backup)?;
    let inspected = Library::inspect_backup(&backup)?;
    checks.check(
        "backup is written and inspectable",
        backup.is_file() && written.info.files == total && inspected.files == total,
        format!(
            "{} files, {} works, {} bytes, format {}",
            inspected.files, inspected.works, written.bytes, inspected.format_version
        ),
    );

    let restored_path = directory.path().join("restored.db");
    let restored = Library::open(&restored_path)?;
    let restore = restored.restore_from(&backup)?;
    let restored_total = restored.count(&LibraryQuery::default())?;
    checks.check(
        "restore reproduces the catalogue",
        restored_total == total && restore.info.files == total,
        format!("{restored_total} of {total} rows"),
    );

    // Restoring over a library that already holds data must leave a safety copy.
    let over = restored.restore_from(&backup)?;
    checks.check(
        "restoring over data keeps a safety copy",
        over.previous_saved_to.is_some(),
        over.previous_saved_to
            .unwrap_or_else(|| "none written".into()),
    );

    println!();
    println!(
        "{} passed, {} failed, {} skipped",
        checks.passed, checks.failures, checks.skipped
    );
    if checks.failures == 0 {
        Ok(())
    } else {
        std::process::exit(1);
    }
}
