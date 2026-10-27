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
| Authorized media roots (F01) | core | host (Home grants a folder) | — |
| Bounded incremental scan (F02) | core | host (grant + scan, cancel, status) | — |
| Work/season/episode/edition model (F03) | core | host (rendered in browse/detail) | — |
| Filename matching (F04) | core (filename only) | host | — |
| Whole-library enrichment (F05) | — | — | — |
| Offline artwork bytes (F06) | — | — | — |
| Player (F07) | state machine | host: real frames in the window, correct aspect, play/pause/seek/resume, **30 fps at 1080p measured**; audio clock 0.982× real time and `avsync` 0.0 ms, but not yet heard (G3b) | — |
| Profiles and progress (F08) | core | host (durable checkpoints, resume, watched) | — |
| Home/Continue Watching (F09) | core query | host | — |
| Next episode rule (F10) | core (`next_episode`) | host (autoplay chain; a gap stops it, an unreadable episode is refused with its reason) | — |
| Search/filter/sort (F11) | core | host (search field, kind/genre/list filters, 4 sorts, paging) | — |
| Favourites/watchlist (F12) | core | host (toggle keys and filters) | — |
| Tracks and subtitles (F13) | track choice stored as a preference | host (audio and subtitle pickers from libmpv's own list, explicit “Off”; styling open) | — |
| Skip intro/credits (F14) | domain type only | — | — |
| Subtitle provider (F15) | — | — | — |
| Local recommendations (F16) | domain type only | — | — |
| Browse previews (F17) | — | — | — |
| PiP / mini-player (F18) | — | — | — |
| Casting / DLNA (F19) | — | — | — |
| Backup and recovery (F20) | core (staged `VACUUM INTO`, validated restore, previous library kept) | host (backup/restore in Settings, both asking before acting) | — |
| Settings and secrets (F21) | core (typed preferences, BCP-47 validation) | host (profiles, autoplay, language, folders) | — |
| Failure/availability states (F22) | core codes | host (refuses an unreadable file and names the recovery) | — |
| Source directory (F23) | — | — | — |
| Acquisition handoff (F24) | — | — | — |
| Local conversion (F25) | — | — | — |

A `core` cell means the rule exists and is tested in Rust: it does **not** mean a
user interface exposes it. The `host` cells are the ones a person can actually
operate today, and only on macOS.

## Target build status

| Target | Source in repo | Build | Device |
| --- | --- | --- | --- |
| MacBook Air M4 (macOS ARM64) | `apps/desktop` | builds and runs; `cargo test --workspace` green (149 tests); `scripts/package-macos.sh` produces a runnable `dist/Screenroom.app` | **running application verified**: playback at 30 fps, resume, tracks, library and Settings UI, bundle launched outside cargo (see `IMPLEMENTATION_STATUS.md`) |
| Windows desktop, Intel Arc B580 | `apps/desktop` (same crate) | not compiled on Windows; no D3D11/shared-texture bridge yet | **BLOCKED** — no access to the machine from this session |
| Samsung UA50AU8000RXER (Tizen 6.0) | `apps/tv` (not started) | not started; Tizen Studio CLI present | **BLOCKED** — no device access and no local signing profile |
| Linux, LG webOS, Android TV, iOS, Android | — | **retired** by ADR-003/ADR-004 | not applicable |

## Per-device capabilities that must never be assumed

Container extension ≠ codec support. 1080p UI coordinates ≠ 4K playback. A remote
image URL ≠ offline artwork. A USB drive on one device ≠ visible on another. A
mini-window ≠ OS PiP. DLNA ≠ every cast protocol. A successful scan ≠ correct
identity. A green unit test ≠ device acceptance. A screenshot of the owner's other
application ≠ Screenroom evidence.

Each of these is recorded per device as `supported`, `unsupported (reason)` or
`unverified` in the platform adapter — never as an optimistic default.
