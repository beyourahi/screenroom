---
title: Browse stalled because the facet query was unindexed
author: Beyourahi
date: 2026-10-28
branch: main
cost: { minutes: 40 }
id: l-b8847377
scope: repo
status: active
src: agent
touches:
  - crates/screenroom-core/src/schema.rs
  - crates/screenroom-core/src/library/mod.rs
  - crates/screenroom-core/examples/library_bench.rs
---

`known_genres` (called from the desktop Browse refresh, apps/desktop/src/app.rs) ran
`SELECT DISTINCT w.genres FROM works w WHERE EXISTS(… editions e JOIN files f …)`.
`editions.work_id`, `files.edition_id`, `file_episodes.episode_id` had no indexes, so
SQLite re-scanned the child table once per work: 44.2 s at 10,000 entries in a
release build, against 13.7 ms once schema 4 adds the four join indexes. The paging
budget in `tests/scale.rs` never caught it because the benchmark only measured
`items`/`count`, and the test library's works have empty genres, so `DISTINCT` still
forces the EXISTS to run for every row — a synthetic library does reproduce it, which
is why `library_bench` now times `known_genres` and `continue_watching`.

Two traps worth keeping: a fixture that builds a "schema N" database by slicing
`schema::CREATE[..len-1]` silently breaks when statements are appended to the end —
name the boundary (artwork is now `len-5`) instead of counting from the end. And a
migration arm per version is not optional bookkeeping: bumping SCHEMA_VERSION without
a `3 =>` arm turns every existing library into `future_database` on open.
