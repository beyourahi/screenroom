# Implementation plan and roadmap

> **2026-10-26:** rewritten for the Rust-first architecture
> ([ADR-002](ADR-002_RUST_FIRST_STACK.md)). The previous Flutter milestone plan is
> history; its acceptance ideas are reused where they still apply.

Every milestone below is **independently testable and resumable**: it has an
explicit entry condition, a definition of done, the evidence that proves it, and a
record in [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md). A milestone is
never "done" because code exists.

Legend for each milestone: **Depends on** (must be satisfied first), **Done when**
(acceptance), **Evidence** (what must be produced).

---

## M0 — Foundation and feasibility ✅ host-verified

**Goal:** prove the risky seams before committing to large amounts of code.

**Contents**
- Rust workspace with edition 2024, one root `Cargo.toml`, committed `Cargo.lock`.
- `screenroom-core` skeleton, `screenroom-player` runtime boundary, `apps/desktop`
  shell.
- libmpv client API smoke (dynamic load, core, options, command, event, property).
- wgpu bridge design compiled against iced 0.14 + wgpu 27.
- ADR-002, architecture, reuse ledger, gates, licensing, roadmap.

**Done when** `cargo test --workspace`, `cargo clippy --workspace --all-targets`
and `cargo fmt --all --check` pass, and the libmpv client-API smoke opens a real
file on this host.

**Evidence** [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md#m0).

---

## M1 — Shared core library ✅ host-verified

**Goal:** every platform-agnostic rule the product depends on, in Rust, with tests.

**Contents**
- Sources: grant, canonicalize, overlap rejection, relabel, relink, remove,
  re-grant restores the same identity.
- Scanning: bounded BFS, cancellation, settle window, sidecar detection, symlink
  refusal, partial issues, complete-only availability withdrawal, single-scan lock.
- Ingestion: incremental rescan, replaced-file editions, small-file rename
  identity, unsafe locator rejection, work/season/episode/file_episode linking.
- Library projection: paged queries, literal search escaping, kind/genre/season
  filters, work grouping, stable sorts, genre picker bound.
- Playback state: sessions, generations, monotonically ordered checkpoints,
  seek generations, ended semantics, stale-callback rejection.
- Lists and progress: favourites, watchlist, explicit watched, hide-from-continue,
  Continue Watching ordering, next-episode gap safety.
- Schema: shared `user_version = 2`, fail-closed migrations, integrity check,
  crash-state normalisation.

**Done when** the integration suites in `crates/screenroom-core/tests/` pass and
the SQLite layout opens a library written by the previous candidate.

**Evidence** [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md#m1).

---

## M2 — Desktop playback slice (next)

**Goal:** one real file plays in the desktop app and resumes after a restart.

**Depends on** M1.

**Contents**
1. Player worker thread in `crates/screenroom-player`: mpv event loop mapped onto
   `PlayerState`, monotonic event sequence, `Arc<Vec<u8>>` frames with generation
   numbers, seek handling (`begin_seek`), end-of-file, error mapping to
   `PlaybackOutcome`.
2. Software render bridge wired to `video_surface.rs`: libmpv SW renderer →
   frame → wgpu texture → iced. Letterbox by default.
3. Player screen in `apps/desktop`: open from a library item, play/pause, seek,
   keyboard and D-pad-less desktop controls, Back returns to the library.
4. Durable resume: `start_session` on open, checkpoint on pause/seek/stop/idle,
   `end_session` on close; resume position shown and honoured.
5. Failure states: missing file, permission lost, unsupported stream, decoder
   failure — each with a truthful message and a recovery action.

**Done when** on this development host a real local file plays with audio and
correct aspect, seeks, and resumes at the stored position after quitting and
relaunching with the network disabled.

**Evidence** command transcript plus a recorded duration/position pair; a
screenshot is optional and never a substitute. Windows and 4K/HDR remain BLOCKED
until that hardware exists.

**Not in M2:** subtitles/track selection, full library UI, design-system styling
of the player, previews, PiP.

---

## M3 — Desktop core UI and design-system mapping

**Goal:** the desktop app is usable as a real library, not a test harness.

**Depends on** M2.

**Contents**
- Dropout Design System mapping for iced: semantic colour roles, type scale,
  spacing, pill actions, focus treatment, motion rules — behind a trait so a
  public build without the private pin still compiles.
- Home: Continue Watching first, then library; empty and partially-scanned states.
- Library: paged grid/list, search, kind/genre/season filters, sorts, virtualised
  rendering with a bounded page window.
- Detail: work/series/season structure, editions, progress, mark watched,
  favourite, watchlist, resume/restart.
- Next episode: verified sequence, cancelable countdown, checkpoint before
  advancing, no gap skipping.
- Tracks and subtitles: embedded/external, language preference, size/offset where
  the engine supports it, explicit limitation copy.
- Profiles: create/rename/remove with a last-profile guard; per-profile progress.
- Settings: playback, language, reduced motion, preview opt-in, provider consent,
  backup/restore entry points.
- Backup and recovery: versioned JSON export/import with validation and
  transactional merge (port of the previous candidate's validated format).
- Error surfaces for every `CoreError` code the UI can reach.

**Done when** a 10,000-item generated library browses, searches and resumes within
recorded frame/input budgets, and a kill-during-playback relaunch loses at most the
last bounded checkpoint.

**Evidence** measured timings on this host, query/browse tests, and the DS mapping
document.

---

## M4 — iOS and Android

**Goal:** the same library and progress on both phones, with native playback.

**Depends on** M1 (core API is stable) and can proceed in parallel with M2/M3.

**Contents**
1. `crates/screenroom-ffi`: UniFFI 0.32 interface over `screenroom-core`
   (sources, scan control, paged queries, sessions/checkpoints, lists), generated
   Swift and Kotlin bindings, committed generation script (bindings are generated,
   not hand-edited).
2. `apps/ios`: SwiftUI browse/detail/player, AVPlayer + AVPlayerLayer, PiP where
   the OS supports it, file import/bookmark scoped access, background audio/state
   restoration.
3. `apps/android`: Compose browse/detail/player, Media3 `ExoPlayer`/`PlayerView`,
   Storage Access Framework tree grants, foreground playback service, TV variant
   with D-pad focus.
4. Contract tests: the same scenario suite run against the Rust core directly and
   through the bindings, asserting identical observable behaviour.

**Done when** each app scans an authorized folder, plays a local file with native
controls, and restores progress after a cold start with the network disabled.

**Evidence** on-device logs for iOS and Android plus binding generation output.
Simulator/emulator results are labelled as such.

---

## M5 — TV platforms

**Goal:** a signed Samsung package that plays local USB media; then webOS and
other platforms.

**Depends on** M1 (contract frozen) and an available device for each acceptance.

**Contents**
1. TV contract harness: a TS package that encodes the core invariants (identity,
   paging, progress ordering, availability) and runs as unit tests against the TV
   storage adapter, mirroring the Rust tests.
2. `apps/tv` (Lightning 3 + Blits + TypeScript):
   - storage adapter (SQLite where the platform allows it, documented fallback
     otherwise), schema parity with `screenroom-core`,
   - D-pad focus graph, Back hierarchy, focus restoration,
   - AVPlay-backed player for Tizen; platform player for webOS,
   - presentation adapter mapping Dropout semantics onto Blits,
   - build targets: legacy-compatible bundle (Chrome 38+/WPEWebKit floor).
3. Packaging: signed `.wgt` for Samsung, `.ipk` for webOS; certificates stay out
   of Git; documented install/remove/update procedure including data-loss warning.
4. Physical TV gates: install, USB enumeration, playback, seek, lifecycle, durable
   state/artwork after power cycle.

**Done when** the physical gates in
[`FEASIBILITY_GATES.md`](FEASIBILITY_GATES.md) pass on the UA50AU8000RXER, or are
explicitly recorded as BLOCKED with the reason.

**Evidence** device receipts with model, firmware, package hash and observed
behaviour.

---

## M6 — Advanced features

**Depends on** the platform that hosts each capability.

| Feature | Primary platform | Content |
| F06 artwork | all | permitted local image bytes, provenance, dimensions, hash, eviction; remote retention only with rights |
| F05 enrichment | desktop/mobile | whole-library durable jobs, visible-first priority, retry/quota/offline queue, manual binding that nothing overwrites |
| F15 subtitle provider | desktop/mobile | optional provider search/download with quota display and manual fallback |
| F14 markers | all | manual intro/credits intervals validated against edition duration; skip only inside a verified interval |
| F16 recommendations | desktop/mobile | explainable local ranking over owned, available media; no hosted AI |
| F17 previews | desktop | single muted decoder, delay, resource cap, reduced-motion off switch |
| F18 PiP | desktop/mobile/TV | true OS PiP where supported, otherwise a clearly labelled mini-player |
| F19 casting | desktop | opt-in, session-scoped DLNA; no always-on endpoint |
| F23 directory | all | user-entered bookmarks with provenance, rights and last-checked status |
| F24 acquisition | desktop | lawful external handoff; no scraping or embedded torrent client |
| F25 conversion | desktop | FFprobe inspection, remux/audio-only/H.264, estimates, progress, cancel, lineage, untouched original |

**Done when** each capability is either verified on its platform or recorded with
a truthful reason and a recovery path.

---

## M7 — Release hardening

- Licensing audit of every shipped native binary (libmpv/FFmpeg build options,
  notices, source offer) and of the private DS boundary.
- Migration and backup drills on each platform, including a corrupted database.
- Performance and resource measurements: cold/warm start, first frame, 4K decode,
  scan of a 10k-item tree, memory ceiling, TV frame pacing.
- Accessibility: contrast, focus visibility at couch distance, subtitle scaling,
  reduced motion.
- Installers/packages per platform; no automated store submission.
- Public-build contract: the product must build and run without undisclosed
  proprietary source.

---

## Feature → milestone map

| PRD | Feature | Primary milestone | Status |
| --- | --- | --- | --- |
| F01 | Sources and grants | M1 (core) + M3/M4/M5 (UI per platform) | core done |
| F02 | Discovery/scanning | M1 | core done |
| F03 | Structured library | M1 | core done |
| F04 | Identity matching | M1 (conservative), M6 (enrichment) | core done, no provider |
| F05 | Whole-library enrichment | M6 | not started |
| F06 | Offline artwork | M6 | not started |
| F07 | Player | M2 (desktop), M4, M5 | boundary + bridge compile |
| F08 | Profiles and progress | M1 (store), M3 (UI) | store done |
| F09 | Home and resume | M1 (query), M3 (UI) | query done |
| F10 | Next episode | M1 (rule), M3 (countdown UI) | rule done |
| F11 | Search/filter/sort | M1 (query), M3 (UI) | query done |
| F12 | Watchlists/favourites | M1 (store), M3/M4/M5 (UI) | store done |
| F13 | Tracks and subtitles | M3 (desktop), M4, M5 | not started |
| F14 | Skip intro/credits | M6 | not started |
| F15 | Subtitle search | M6 | not started |
| F16 | Local recommendations | M6 | not started |
| F17 | Browse previews | M6 | not started |
| F18 | PiP / mini-player | M6 | not started |
| F19 | Casting | M6 | not started |
| F20 | Backup and recovery | M3 (port of validated format) | not started in Rust |
| F21 | Settings and secrets | M3 | not started |
| F22 | Failure states | M2/M3 (desktop), M4/M5 | core codes done |
| F23 | Source directory | M6 | not started |
| F24 | Acquisition handoff | M6 | not started |
| F25 | Local conversion | M6 | not started |

## Why this order

Playback is the product's core promise and its riskiest integration, so M2 follows
the core immediately: it is the only milestone that can invalidate the desktop
stack. Mobile (M4) and TV (M5) come after the core API is exercised by a real
client, so their bindings and contract harness are written against behaviour that
is already proven. Advanced features (M6) depend on durable artwork, enrichment and
playback working on at least one platform; building them earlier would prove
nothing.
