# Screenroom

A cinematic, offline-first local media application: your own movies and series,
organized like a streaming catalog — continue watching, next episode, profiles,
search, subtitles — running entirely on the device that holds the files.

**Stack (final, 2026-10-26):** Rust-first.
[`docs/ADR-002_RUST_FIRST_STACK.md`](docs/ADR-002_RUST_FIRST_STACK.md) is
authoritative. The earlier Flutter-first decision
([ADR-001](docs/ADR-001_SELECTED_STACK.md)) is superseded and kept as history.

| Platform | UI | Playback |
| --- | --- | --- |
| Windows / Linux / macOS | Rust + iced + wgpu | libmpv |
| iOS | SwiftUI | AVPlayer |
| Android / Android TV | Jetpack Compose | Media3 |
| Samsung Tizen / LG webOS | LightningJS 3 + Blits + TypeScript | platform-native (AVPlay etc.) |
| Fallback only | Flutter + Dart | per-device native |

Shared domain: Rust. Database: SQLite where supported. Media work: FFmpeg/FFprobe.
Design: private Dropout Design System, adapted per platform. License: MIT for
first-party code.

## Status

- **Implemented and host-verified:** the shared Rust core (sources, bounded
  scanning, ingestion, paged library queries, playback sessions/progress, lists,
  shared SQLite schema) and the libmpv runtime boundary, including a real
  load-and-decode smoke test.
- **Compiles, not yet proven:** the iced + wgpu desktop shell and the libmpv → wgpu
  video bridge (no frame presented on hardware yet).
- **Blocked:** every physical-device gate (Windows, Samsung Tizen, iOS, Android).
  None has been claimed.

Live detail, receipts and blockers:
[`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md).

## Run it

```bash
cargo test --workspace                                  # 53 tests today
cargo run -p screenroom-player --example capability      # libmpv on this device
cargo run -p screenroom-desktop                          # desktop shell (M1 skeleton)
```

`SCREENROOM_LIBRARY=/path/to/library.db` points the desktop shell at a real
library; without it the shell uses a throwaway in-memory library.

## Read before coding

1. [`AGENTS.md`](AGENTS.md) — engineering rules and evidence standards.
2. [`docs/PRD.md`](docs/PRD.md) — product requirements F01–F25 (unchanged by the
   stack switch).
3. [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — how the pieces fit.
4. [`docs/IMPLEMENTATION_PLAN.md`](docs/IMPLEMENTATION_PLAN.md) — milestones M0–M7.
5. [`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md) — what is done,
   verified, blocked.
6. [`docs/FEASIBILITY_GATES.md`](docs/FEASIBILITY_GATES.md) — device gates.
7. [`docs/REUSE_LEDGER.md`](docs/REUSE_LEDGER.md) — what was ported, kept or dropped.
8. [`docs/DATA_MODEL.md`](docs/DATA_MODEL.md),
   [`docs/DESIGN.md`](docs/DESIGN.md),
   [`docs/DROPOUT_DESIGN_INTEGRATION.md`](docs/DROPOUT_DESIGN_INTEGRATION.md),
   [`docs/LICENSING.md`](docs/LICENSING.md).

## Repository layout

```text
Cargo.toml                  # Rust workspace
crates/screenroom-core/     # domain, SQLite library, scanner, fingerprint
crates/screenroom-player/   # libmpv runtime boundary, player state machine
apps/desktop/               # iced + wgpu client, video_surface.rs bridge
docs/                       # ADRs, architecture, roadmap, ledger, gates
reference/                  # historical Svelte/Tauri prototype and handoff
.worktrees/                 # archived agent worktrees (Git-ignored, recoverable)
```

## Hard constraints

- One repository for all first-party code; no submodules or sibling repos.
- Installed applications only — no website, PWA, browser tab or required server.
- Offline-first; no paid service, account, cloud or always-on host is required.
- User media is read in place and never modified, moved or deleted.
- No scraping, access-control bypass or unauthorized acquisition.
- Never claim a device test that did not happen.
