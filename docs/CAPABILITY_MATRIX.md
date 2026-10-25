# Platform capability matrix

> **2026-10-26:** rewritten for the Rust-first stack
> ([ADR-002](ADR-002_RUST_FIRST_STACK.md)). The Flutter-era matrix is history.

**Legend**
`core` = implemented in `screenroom-core` and covered by host tests ·
`host` = implemented and exercised on the macOS development host ·
`—` = not built yet ·
`BLOCKED` = needs hardware/tooling that is unavailable ·
`proven` = verified on a physical device (none yet).

| Capability | Core | Desktop (iced) | iOS | Android / TV | Tizen TV | webOS |
| --- | --- | --- | --- | --- | --- | --- |
| Authorized media roots (F01) | core | — | — | — | — | — |
| Bounded incremental scan (F02) | core | — | — | — | — | — |
| Work/season/episode/edition model (F03) | core | — | — | — | — | — |
| Filename/NFO matching (F04) | core (filename only) | — | — | — | — | — |
| Whole-library enrichment (F05) | — | — | — | — | — | — |
| Offline artwork bytes (F06) | — | — | — | — | — | — |
| Player (F07) | state machine | bridge compiles | — | — | — | — |
| Profiles and progress (F08) | core | — | — | — | — | — |
| Home/Continue Watching (F09) | core query | — | — | — | — | — |
| Next episode rule (F10) | core | — | — | — | — | — |
| Search/filter/sort (F11) | core | — | — | — | — | — |
| Favourites/watchlist (F12) | core | — | — | — | — | — |
| Tracks and subtitles (F13) | — | — | — | — | — | — |
| Skip intro/credits (F14) | — | — | — | — | — | — |
| Subtitle provider (F15) | — | — | — | — | — | — |
| Local recommendations (F16) | — | — | — | — | — | — |
| Browse previews (F17) | — | — | — | — | — | — |
| PiP / mini-player (F18) | — | — | — | — | — | — |
| Casting / DLNA (F19) | — | — | — | — | — | — |
| Backup and recovery (F20) | — | — | — | — | — | — |
| Settings and secrets (F21) | — | — | — | — | — | — |
| Failure/availability states (F22) | core codes | partial | — | — | — | — |
| Source directory (F23) | — | — | — | — | — | — |
| Acquisition handoff (F24) | — | — | — | — | — | — |
| Local conversion (F25) | — | — | — | — | — | — |

"Core" cells marked `core` mean the rule exists and is tested in Rust. They do not
mean any user interface exposes it yet.

## Platform build status

| Platform | Source in repo | Build | Device |
| --- | --- | --- | --- |
| macOS ARM64 | `apps/desktop` | `cargo check` passes; libmpv present; no signed app bundle (no full Xcode) | not run |
| Windows | `apps/desktop` (same crate) | not compiled on Windows | BLOCKED (no hardware) |
| Linux | `apps/desktop` (same crate) | not compiled on Linux | not run |
| iOS | — | not started | BLOCKED (no full Xcode) |
| Android / Android TV | — | not started | BLOCKED (no verified SDK) |
| Samsung Tizen | — | not started | BLOCKED (device/SDK) |
| LG webOS | — | not started | not run |

## Per-device capabilities that must never be assumed

Container extension ≠ codec support. 1080p UI coordinates ≠ 4K playback. A remote
image URL ≠ offline artwork. A USB drive on one device ≠ visible on another. A
mini-window ≠ OS PiP. DLNA ≠ every cast protocol. A successful scan ≠ correct
identity. A green unit test ≠ device acceptance.

Each of these is recorded per device as `supported`, `unsupported (reason)` or
`unverified` in the platform adapter — never as an optimistic default.
