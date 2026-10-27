# Implementation plan and roadmap

> **2026-10-27:** rewritten for the Rust-first architecture
> ([ADR-002](ADR-002_RUST_FIRST_STACK.md)), scoped to the owner's three devices by
> [ADR-004](ADR-004_PERSONAL_EDITION_SCOPE.md) (which amends
> [ADR-003](ADR-003_DESKTOP_AND_TV_SCOPE.md)). The previous Flutter milestone plan
> is history; its acceptance ideas are reused where they still apply.

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

## M2 — Desktop playback slice ✅ host-verified on macOS

**Goal:** one real file plays in the desktop app and resumes after a restart.

**State 2026-10-27:** contents 1–5 are implemented, and a frame has been observed
in the running window: correct pillarbox/letterbox aspect, moving frames, play,
pause, seek (±10 s), resume across a quit and relaunch, and durable checkpoints.
Measured at 1080p: **30.0 fps published with zero drops** (the media's own rate)
at a 3440×1440 window. See
[`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md#m2--desktop-playback-slice).

**Audio and tracks (2026-10-28):** audio is now measured rather than assumed —
the audio clock advances at 0.982× wall time on a generated fixture, libmpv reports
`avsync` at **0.0 ms** with a 12 ms cumulative adjustment, and the source's own
volume is −24.1 dB. Audio and subtitle tracks are enumerated with their language and
title, switched through libmpv, and the selection is read back from the snapshot so
a refused switch is visible. See
[`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md#audio-and-sync-measured-2026-10-28).

**Outstanding, named rather than implied:**

- Nobody has yet *heard* the output. The measurement above is instrument evidence;
  gate G3b still needs a listening pass (the owner's, on real speakers).
- The offline run was proven by holding zero network sockets rather than by
  disabling the machine's networking; an airplane-mode run is still owed.
- Windows and 4K/HDR remain BLOCKED on hardware.

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
relaunching with the network disabled. **Met except for the three named items
above.**

**Evidence** command transcript, database rows, window screenshots carrying the
fixture's burnt-in timecode, and the automated frame-rate and resume tests.
Windows and 4K/HDR remain BLOCKED until that hardware exists.

**Not in M2:** subtitles/track selection, full library UI, design-system styling
of the player, previews, PiP.

---

## M3 — Desktop core UI and design-system mapping ⏳ essential surface delivered

**Goal:** the desktop app is usable as a real library, not a test harness.

**State 2026-10-28:** delivered on macOS — Home with Continue Watching and folder
granting; the paged library with search, kind and list filters, sorts and paging; the
detail page with resume/restart and the flag actions; the player with audio and
subtitle pickers and next-episode autoplay; keyboard and mouse navigation; the
empty/loading/error surfaces; viewer profiles; a Settings screen covering
preferences, folders (rescan, relink, remove) and backup/restore; and the private
Dropout Design System adapter with its Git-ignored pin. The 10,000-item **store**
budget is measured (slowest page 43.5 ms, ingest 825 ms).

Still open: subtitle styling (size/offset), browse previews, and a measured
rendering/input budget at 10,000 items.

**Depends on** M2 (satisfied).

**Contents**
- ✅ Dropout Design System mapping for iced: semantic colour roles, type scale,
  spacing, pill actions, focus treatment — behind a build-time include so a clean
  checkout without the private pin still compiles.
- ✅ Home: Continue Watching first, then library; empty and partially-scanned states.
- ✅ Library: paged grid, search, kind/list filters, sorts, bounded page window.
- ✅ Detail: work/season structure, editions, progress, mark watched, favourite,
  watchlist, resume/restart.
- ✅ Next episode: verified sequence from the core, gap stops the chain, unreadable
  next episode refused with its reason. No cancelable countdown — it advances.
- ✅ Tracks and subtitles: enumerated from libmpv with language/title, an explicit
  “Off” for subtitles, preference stored. ⏳ size/offset styling.
- ✅ Profiles: create/rename/remove with the last-profile guard; per-profile progress.
- ✅ Settings: autoplay, language, folders (rescan/relink/remove), backup/restore.
  ⏳ reduced-motion and preview opt-in (they belong with previews).
- ✅ Backup and recovery: SQLite `VACUUM INTO` through a staging file, validated
  restore that keeps the previous library beside the new one.
- ✅ Error surfaces for the `CoreError` codes the UI can reach, each naming a
  recovery action.

**Deliberate change of format:** the archived candidate used versioned JSON for
backup. This build uses a validated SQLite copy instead, because the whole product
state already *is* SQLite and a JSON round-trip would have to re-derive every
foreign key. The invariants that mattered — validation before writing, no partial
file, transactional swap, refuse a foreign or newer file — are all tested.

**Done when** a 10,000-item generated library browses, searches and resumes within
recorded frame/input budgets, and a kill-during-playback relaunch loses at most the
last bounded checkpoint.

**Evidence** measured timings on this host, query/browse tests, and the DS mapping
document.

---

## M4 — Removed (mobile apps out of scope)

The former M4 (iOS and Android phone/tablet apps with UniFFI bindings) was removed
on 2026-10-26 by [ADR-003](ADR-003_DESKTOP_AND_TV_SCOPE.md). Nothing had been
built. Android TV moved into M5. The number is kept so existing references stay
stable.

---

## M5 — Samsung Tizen TV (the only TV target)

**Goal:** a LightningJS 3 + Blits client that plays the owner's local media on the
Samsung UA50AU8000RXER (Tizen 6.0). LG webOS and Android TV were retired by
[ADR-004](ADR-004_PERSONAL_EDITION_SCOPE.md); Apple TV and Roku were never in
scope.

**Depends on** M1 (contract frozen) and access to the television.

**Contents**
1. TV contract harness: a TS package that encodes the core invariants (identity,
   paging, progress ordering, availability) and runs as unit tests against the TV
   storage adapter, mirroring the Rust tests.
2. `apps/tv` (Lightning 3 + Blits + TypeScript):
   - storage adapter (SQLite where the platform allows it, documented fallback
     otherwise), schema parity with `screenroom-core`,
   - D-pad focus graph, Back hierarchy, focus restoration,
   - AVPlay-backed player,
   - presentation adapter mapping Dropout semantics onto Blits,
   - build target: a bundle the Tizen 6.0 engine runs.
3. Packaging: a locally signed `.wgt`; certificate material stays out of Git;
   documented install/remove/update procedure including the data-loss warning.
4. Physical gate: install, USB enumeration, playback, seek, lifecycle, durable
   state/artwork after a power cycle.

**Done when** gate G5 passes on the UA50AU8000RXER, or is explicitly recorded as
BLOCKED with the reason (today: device access and a signing certificate, both
requiring the owner).

**Evidence** a device receipt with model, firmware, package hash and observed
behaviour.

---

## M6 — Advanced features

**Depends on** the platform that hosts each capability.

| Feature | Primary platform | Content |
| --- | --- | --- |
| F06 artwork | all | permitted local image bytes, provenance, dimensions, hash, eviction; remote retention only with rights |
| F05 enrichment | desktop/TV | whole-library durable jobs, visible-first priority, retry/quota/offline queue, manual binding that nothing overwrites |
| F15 subtitle provider | desktop/TV | optional provider search/download with quota display and manual fallback |
| F14 markers | all | manual intro/credits intervals validated against edition duration; skip only inside a verified interval |
| F16 recommendations | desktop/TV | explainable local ranking over owned, available media; no hosted AI |
| F17 previews | desktop | single muted decoder, delay, resource cap, reduced-motion off switch |
| F18 PiP | desktop/TV | true OS PiP where supported, otherwise a clearly labelled mini-player |
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
| F01 | Sources and grants | M1 (core) + M3/M5 (UI per platform) | core + desktop grant/scan UI |
| F02 | Discovery/scanning | M1 | core done |
| F03 | Structured library | M1 | core done |
| F04 | Identity matching | M1 (conservative), M6 (enrichment) | core done, no provider |
| F05 | Whole-library enrichment | M6 | not started |
| F06 | Offline artwork | M6 | not started |
| F07 | Player | M2 (desktop), M5 (TV) | **desktop verified on macOS** (30 fps, aspect, transport, resume); no audio-by-ear or a/v-sync figure yet |
| F08 | Profiles and progress | M1 (store), M3 (UI) | store + desktop UI: durable progress, resume, watched |
| F09 | Home and resume | M1 (query), M3 (UI) | delivered on desktop |
| F10 | Next episode | M1 (rule), M3 (countdown UI) | rule + desktop autoplay chain (gap-safe, refusable) |
| F11 | Search/filter/sort | M1 (query), M3 (UI) | delivered on desktop |
| F12 | Watchlists/favourites | M1 (store), M3/M5 (UI) | delivered on desktop |
| F13 | Tracks and subtitles | M3 (desktop), M5 (TV) | desktop: enumerate and switch both, “Off” for subtitles; styling open |
| F14 | Skip intro/credits | M6 | not started |
| F15 | Subtitle search | M6 | not started |
| F16 | Local recommendations | M6 | not started |
| F17 | Browse previews | M6 | not started |
| F18 | PiP / mini-player | M6 | not started |
| F19 | Casting | M6 | not started |
| F20 | Backup and recovery | M3 (port of validated format) | delivered: VACUUM-into staging, validated restore, previous library kept |
| F21 | Settings and secrets | M3 | delivered: typed preferences, profiles, folders, backup entry points |
| F22 | Failure states | M2/M3 (desktop), M5 (TV) | core codes done |
| F23 | Source directory | M6 | not started |
| F24 | Acquisition handoff | M6 | not started |
| F25 | Local conversion | M6 | not started |

## Why this order

Playback is the product's core promise and its riskiest integration, so M2 followed
the core immediately: it was the only milestone that could invalidate the desktop
stack, and it has now been verified on macOS. The Samsung TV (M5) is equally
mandatory for the owner, but it is **BLOCKED on access**, so its contract harness
is written once the core API is exercised by a real client — it mirrors behaviour
that is already proven rather than inventing it. Advanced features (M6) depend on
playback working on at least one platform; building them earlier would prove
nothing.
