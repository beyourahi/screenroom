# Reuse ledger — what survives the Rust-first transition

Audit of everything preserved in the repository before the stack change, with an
explicit verdict per artifact. Verdicts: **ported** (behaviour reimplemented in
Rust, tests carried over), **reused** (kept as-is because it is still correct),
**rewritten** (same intent, new form), **discarded** (not carried forward), or
**deferred** (still wanted, not yet moved).

Nothing here is deleted. `reference/` and the `archive/*` branches are the audit
trail.

---

## 1. Shared domain core (`.worktrees/*/packages/screenroom_core`)

| Artifact | Verdict | Notes |
| --- | --- | --- |
| `models.dart` (Source, Profile, LibraryItem, PlaybackSession, SegmentMarker, BackgroundJob) | **ported** | `crates/screenroom-core/src/domain.rs`; enums keep the exact stored strings (`permissionLost`, `movie/series/unknown`) so an existing library still opens |
| `filename.dart` parse rules | **ported** | `filename.rs`, same regexes, same confidence values, same unknown fallback; covered by 9 unit tests |
| `fingerprint.dart` (size + 64 KiB head/tail SHA-256) | **ported** | `fingerprint.rs`; same sampling and clamp behaviour, 4 tests |
| `scanner.dart` walk, bounds, sidecar and settle rules | **ported** | `scanner.rs`; same caps (250k entries, depth 64, 15 s), issue codes and complete-only availability withdrawal |
| `repository.dart` schema DDL | **reused verbatim** | `schema.rs`; same tables, indexes and `user_version = 2` → existing device libraries open |
| `repository.dart` sources/profiles/queries/progress/lists logic | **ported** | `library/{sources,catalog,playback,lists}.rs`; monotonic `(sequence, seek_generation)` rule preserved exactly |
| `repository.dart` metadata/enrichment/markers/backup/jobs APIs | **deferred** | listed in [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md#deliberately-not-ported-yet) with targets |
| `core_test.dart` assertions | **ported selectively** | 28 Rust integration tests cover the same invariants (identity, rename, replaced edition, stale session, gap safety, availability, migration) |
| `metadata_test.dart`, `recovery_test.dart`, `backup_bundle_test.dart`, `ui_test.dart` | **deferred** | arrive with enrichment (M6) and the desktop UI (M3) |

Behaviour change made during the port: re-granting a previously removed folder
reactivates the same source row instead of creating an orphan. This keeps history
visible after "remove source → add it again" and is covered by a test. Recorded
here because it is a deliberate divergence.

## 2. Services (`.worktrees/*/packages/screenroom_services`)

| Artifact | Verdict | Notes |
| --- | --- | --- |
| `media_tools.dart` (FFprobe/FFmpeg argument arrays, remux, audio-only) | **deferred** to M6 | the argument-array discipline (no shell interpolation) is the part to keep |
| `artwork.dart` / `artwork_cache.dart` (permitted byte storage, provenance, hashing) | **deferred** to M6 | the validated-artwork bundle belongs in `screenroom-core` + adapter |
| `metadata.dart` (TMDB client, quota, provenance) | **deferred** to M6 | logic is reusable; must be rewritten against a Rust HTTP client with the same consent boundary |
| `subtitles.dart` (OpenSubtitles search/hash/quota) | **deferred** to M6 | same |
| `network.dart` (bounded fetch, allowlists) | **deferred** to M6 | security rules carry over verbatim |
| `directory.dart` (source bookmarks with provenance/rights) | **deferred** to M6 | `DirectoryEntry` is already an entity in the schema |
| `casting.dart` (opt-in session-scoped DLNA) | **deferred** to M6 | the "off by default, session-scoped, token-bound" model is kept |
| their Dart tests | **deferred** | port alongside each service |

## 3. Flutter application (`.worktrees/*/apps/screenroom`)

| Artifact | Verdict | Why |
| --- | --- | --- |
| `lib/ui/*`, `lib/player/*` widget code | **discarded** | iced/SwiftUI/Compose replace it; the screens are a design reference only |
| `lib/design/dropout_tokens.g.dart` | **discarded** | generated private values, never committed; the new mapping is per-platform and stays out of Git |
| `lib/platform/native_playback.dart` (media_kit wiring) | **discarded** as code | `screenroom-player` replaces it; the state-machine lessons (session ids, generation tokens) were carried over |
| `lib/platform/tizen_storage.dart`, `main_tizen.dart`, `tizen/` runner | **discarded** as code, **reused** as knowledge | the Tizen privilege/manifest requirements and storage-path lore inform the M5 TV client |
| `windows/`, `macos/`, `linux/` runners | **discarded** | iced owns the window |
| `tool/native_mpv_smoke.dart` | **rewritten** | `crates/screenroom-player/tests/libmpv_smoke.rs` does a stronger check (real file, real duration) |
| `assets/tmdb-logo.svg` | **reused** when TMDB attribution is needed | branding, not MIT; attribution rules still apply |

## 4. Documentation

| Artifact | Verdict | Notes |
| --- | --- | --- |
| `PRD.md` feature register (F01–F25) | **reused** | the product requirements did not change; only the stack did |
| `DATA_MODEL.md` entity and invariant rules | **reused** | the Rust implementation follows it; the DDL matches |
| `DESIGN.md`, `DROPOUT_DESIGN_INTEGRATION.md` | **reused** with a platform-neutral reading | "map semantics per platform", not "map into Flutter" |
| `FEASIBILITY_GATES.md` | **rewritten** | same discipline, new stack and new gates |
| `LICENSING.md` (from the archived worktree) | **rewritten** | Rust/native binary obligations replace Dart/Flutter ones; the libmpv/FFmpeg findings carry over |
| `DECISIONS_AND_RESEARCH.md` | **reused as history** | its provider/technical research is still useful; stack statements are superseded |
| `NAMING.md` | **rewritten** | name finalized |
| `PROTOTYPE_REVIEW.md` | **reused as history** | prototype lessons still hold |
| `TEST_PLAN.md` | **rewritten for the new clients**, test *intent* reused | scenario list (rename, revoked permission, corrupt backup, power loss) kept |
| `CODEX_KICKOFF.md` | **discarded** as instructions | superseded governance; kept for history |
| `tooling/verify.py`, `dependency_licenses.py` | **discarded** as tooling | replaced by `cargo test`/`clippy`/`fmt` and a future `cargo deny`/license task |
| Dropout DS pin `41406ab…` | **reused** | still the canonical immutable pin |

## 5. `reference/` (historical)

| Artifact | Verdict | Why |
| --- | --- | --- |
| `reference/screenroom` (Svelte/Tauri prototype, 24 MB incl. Tauri deps) | **discarded as runtime** | archived in `prototype-v0.1.zip`; the Tauri shell contradicted the installed-app contract and cannot serve TV or mobile |
| `reference/SCREENROOM_CHATGPT_HANDOFF.md` (69 KB) | **reused as product intent** | the original journey and constraint notes still describe the product |
| `reference/prototype-v0.1.zip` | **preserved** | historical evidence, excluded from the build |

## 6. Behaviour that must be re-proven, not inherited

Carrying the code over does not carry the evidence over. These were only ever
"automated" or "host" results, and the rewrite invalidates even the host ones until
re-run in Rust:

- physical Windows playback, GPU decode, 4K/HDR, audio-device switching;
- Samsung install/USB/AVPlay/lifecycle/durable artwork;
- Tizen SQLite availability and app-private storage quotas;
- 10k-item library performance;
- provider terms, quotas and offline-retention rights;
- any libmpv rendering path.

M0/M1 receipts in [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md) are the new
baseline; nothing from the Flutter candidate is cited as device evidence.
