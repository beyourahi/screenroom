# Platform capability matrix

> **2026-10-27:** platforms reduced to the three Personal Edition targets by
> [ADR-004](ADR-004_PERSONAL_EDITION_SCOPE.md). Linux, LG webOS and Android TV
> columns are gone; the Flutter-era matrix is history.

**Legend**
`core` = implemented in `screenroom-core` and covered by host tests ·
`host` = implemented and exercised in the running application on this macOS host ·
`—` = not built yet ·
`BLOCKED` = needs hardware, tooling or credentials that are unavailable ·
`proven` = verified on the physical target device (none yet).

| Capability | Core | Desktop (iced, macOS) | Tizen TV |
| --- | --- | --- | --- |
| Authorized media roots (F01) | core | host (Home grants a folder) | client: removable storage is enumerated and granted through the platform's own API, and a granted root is stored as listener data. **Not yet on a TV** |
| Bounded incremental scan (F02) | core | host (grant + scan, cancel, status) | **not built.** The TV client can see a drive but does not walk it; this is the next TV work item |
| Work/season/episode/edition model (F03) | core | host (rendered in browse/detail) | client: the core's entities are ported as a typed contract (`core/contract.ts`), with the same fields and the same opaque-id identity rule |
| Filename matching (F04) | core (filename only) | host | **not built** — no scan means nothing to interpret yet |
| Whole-library enrichment (F05) | — | — | — |
| Offline artwork bytes (F06) | — | — | — |
| Player (F07) | state machine | host: real frames in the window, correct aspect, play/pause/seek/resume, **30 fps at 1080p measured**; audio clock 0.982× real time and `avsync` 0.0 ms, but not yet heard (G3b) | client: the AVPlay state machine is implemented against the documented call order, with every transition guarded by `getState()`, and its arithmetic and track reading are tested off-device. **No media has ever reached AVPlay** |
| Profiles and progress (F08) | core | host (durable checkpoints, resume, watched) | client: per-work resume points, watched marking and per-screen focus memory, on `localStorage` behind a port. The store's rules — nothing silently reset, one bad row never costs the library, a newer document refused rather than guessed, bounded input — are tested off-device |
| Home/Continue Watching (F09) | core query | host | client: the same predicate as the core (`position > 0 AND watched = 0 AND hidden = 0`, source not removed), ordered by when playback last changed with a stable tie-break, and tested |
| Next episode rule (F10) | core (`next_episode`) | host (autoplay chain; a gap stops it, an unreadable episode is refused with its reason) | — |
| Search/filter/sort (F11) | core | host (search field, kind/genre/list filters, 4 sorts, paging) | — |
| Favourites/watchlist (F12) | core | host (toggle keys and filters) | **not built** on the TV screen |
| Tracks and subtitles (F13) | track choice and subtitle size/position stored as preferences | host (audio and subtitle pickers from libmpv's own list, explicit “Off”; subtitle size 50–300% and position 0–100% via `sub-scale`/`sub-pos`, read back from libmpv) | **partial**: AVPlay's track list is read and parsed into the app's own shape, with tests, but there is no menu yet and no external subtitle support. Embedded text tracks are drawn by the decoder |
| Skip intro/credits (F14) | domain type only | — | — |
| Subtitle provider (F15) | — | — | — |
| Local recommendations (F16) | domain type only | — | — |
| Browse previews (F17) | — | — | — |
| PiP / mini-player (F18) | — | — | — |
| Casting / DLNA (F19) | — | — | — |
| Backup and recovery (F20) | core (staged `VACUUM INTO`, validated restore, previous library kept) | host (backup/restore in Settings, both asking before acting) | — |
| Settings and secrets (F21) | core (typed preferences, BCP-47 validation) | host (profiles, autoplay, language, folders) | client: a device-diagnostics screen that reports what the host actually is — model, Tizen version, playback path, keys the TV accepted, whether storage took a probe write, and where state lives. No storing of anything secret, because there is nothing secret to store |
| Failure/availability states (F22) | core codes | host (refuses an unreadable file and names the recovery) | client: the core's error codes are ported, and every empty screen names the situation it is actually in — no media yet, a drive not mounted, a search too narrow — each with its own remedy rather than one generic message |
| Source directory (F23) | — | — | — |
| Acquisition handoff (F24) | — | — | — |
| Local conversion (F25) | — | — | — |

A `core` cell means the rule exists and is tested in Rust: it does **not** mean a
user interface exposes it. The `host` cells are the ones a person can actually
operate today, and only on macOS. A `client` cell is the TV client's TypeScript,
which is typechecked and covered by 82 off-device tests — **none of it has executed on
a television.** A `client` cell never means "verified on the TV".

## Target build status

| Target | Source in repo | Build | Device |
| --- | --- | --- | --- |
| MacBook Air M4 (macOS ARM64) | `apps/desktop` | builds and runs; `cargo test --workspace` green (189 tests); `scripts/package-macos.sh` produces a runnable `dist/Screenroom.app`, and `scripts/verify-bundle.sh` checks it plus the bundle's own libmpv and all 20 of its dependencies | **running application verified**: playback at 30 fps, resume, tracks, subtitle size/position, library and Settings UI, bundle launched outside cargo (see `IMPLEMENTATION_STATUS.md`) |
| Windows desktop, Intel Arc B580 | `apps/desktop` (same crate) | not compiled on Windows; no D3D11/shared-texture bridge yet. Three Windows-only defects were found by reasoning and fixed from macOS on 2026-10-28 — verbatim-prefix path identity, the libmpv search consulting the working directory, and four fixed grid columns at 3440×1440 — each asserted with Windows-shaped inputs, none observed on Windows | **BLOCKED** — no access to the machine from this session. `WINDOWS_VALIDATION.md` is the exact handoff: prerequisites, commands, per-area acceptance checks, and the check that would falsify each fix |
| Samsung UA50AU8000RXER (Tizen 6.0) | `apps/tv` — LightningJS 3 + Blits + TypeScript: 5 screens (Home, Browse, Detail, Settings, Player), the AVPlay playback port, remote-key registration and a storage port | **builds and packages on macOS**: `pnpm test` 82/82, `pnpm typecheck` clean, `pnpm build` produces a single 428,957-byte bundle, and the signed `.wgt` (147,404 bytes, sha256 `ec40f2ce…`) carries a byte-identical `main.js`. `tizen` CLI 2.5.25 and `sdb` 4.2.36 are present; one signing profile (`screenroom`) exists and is active | **BLOCKED** — no device: `sdb devices` lists nothing attached (checked 2026-10-28). Nothing in this client has run on a Tizen engine, no frame has been rendered, and no media has reached AVPlay. `TIZEN_DEVICE_HANDOFF.md` is the step-by-step handoff |
| Linux, LG webOS, Android TV, iOS, Android | — | **retired** by ADR-003/ADR-004 | not applicable |

## Per-device capabilities that must never be assumed

Container extension ≠ codec support. 1080p UI coordinates ≠ 4K playback. A remote
image URL ≠ offline artwork. A USB drive on one device ≠ visible on another. A
mini-window ≠ OS PiP. DLNA ≠ every cast protocol. A successful scan ≠ correct
identity. A green unit test ≠ device acceptance. A screenshot of the owner's other
application ≠ Screenroom evidence.

Each of these is recorded per device as `supported`, `unsupported (reason)` or
`unverified` in the platform adapter — never as an optimistic default.
