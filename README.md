# Screenroom

**Screenroom Personal Edition** — a cinematic, offline-first local media
application for one person's own library: continue watching, next episode,
profiles, search, subtitles, playing entirely on the device that holds the files.

**Scope (final, 2026-10-27):** exactly three devices.
[`docs/ADR-004_PERSONAL_EDITION_SCOPE.md`](docs/ADR-004_PERSONAL_EDITION_SCOPE.md)
is the current word on platforms; it amends
[ADR-002](docs/ADR-002_RUST_FIRST_STACK.md) (Rust-first stack, still authoritative)
and [ADR-003](docs/ADR-003_DESKTOP_AND_TV_SCOPE.md) (desktop + TV split). The
earlier Flutter decision ([ADR-001](docs/ADR-001_SELECTED_STACK.md)) is history.

| Target | UI | Playback |
| --- | --- | --- |
| MacBook Air M4 (macOS) | Rust + iced + wgpu | libmpv → wgpu (software bridge) |
| Windows desktop, Intel Arc B580 | Rust + iced + wgpu (same crate) | libmpv; Arc decode is the goal |
| Samsung UA50AU8000RXER (Tizen 6.0) | LightningJS 3 + Blits + TypeScript | Tizen AVPlay |

Linux, LG webOS, Android TV, phones/tablets, Apple TV and Roku are out of scope, as
is public distribution or any paid service. Shared domain: Rust. Database: SQLite.
Media work: FFmpeg/FFprobe. Design: private Dropout Design System, adapted per
platform. License: MIT for first-party code.

## Status

**Verified on the macOS host** (MacBook Air M4, macOS, Apple Silicon):

- The shared Rust core — sources, bounded cancellable scanning, ingestion, paged
  library queries, playback sessions and progress, lists, SQLite schema.
- libmpv through the app: a real fixture decodes **and reaches the window** with
  correct pillarbox aspect and moving frames; play, pause, seek (±10 s) and resume
  all behave; progress is durable across a quit and relaunch; a finished file
  restarts from the beginning and says so in the UI.
- The desktop application's screens: Home with Continue Watching, library browse
  with paging, search, filters, sorting, a detail page and the player, driven by
  keyboard and mouse, with empty/loading/error states; audio and subtitle track
  pickers; next-episode autoplay; viewer profiles and a Settings screen for
  preferences, folders and backup/restore.
- An installable macOS bundle: `scripts/package-macos.sh` produces
  `dist/Screenroom.app` carrying its own libmpv, and it was launched and driven
  outside cargo.
- **Measured:** the software bridge sustains **30.0 fps at 1080p with the playhead
  at 1.00×**, in both debug and the release build that ships. Release cost at
  1080p: ~66–76 % of one core and a flat ~255 MB RSS during playback, falling to
  ~3 % CPU and 108 MB once it ends. The process holds no network sockets.

**Measured rather than assumed:** the audio clock runs at **0.982× real time** on a
known audio track and libmpv reports **0.0 ms `avsync`** (12 ms cumulative
adjustment, no mistimed frames); audio and subtitle tracks are enumerated and
switched, with the selection read back from the player.

**Not yet verified:**

- Nobody has *heard* the output through speakers. The instrument evidence above is
  not a listening test, so gate G3b stays partial.
- The offline run was proven by holding zero network sockets, not by disabling
  this machine's networking.
- 4K and hardware decode are neither claimed nor measured: the software bridge is
  `hwdec=no` by design and bounded to 1920 px.
- Everything Windows and everything Samsung TV. Both are **BLOCKED** on hardware
  access and (for Tizen) a local signing certificate. No device receipt exists.

Full detail, receipts and blockers:
[`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md).

## Run it

```bash
cargo test --workspace          # 168 tests, 0 failures on the dev host
cargo clippy --workspace --all-targets
cargo fmt --all --check
cargo run -p screenroom-desktop                    # the application
```

Measurements and checks that need **no window** — prefer these, they never take
over the desktop (see "Testing without taking over the desktop" in
[`AGENTS.md`](AGENTS.md)):

```bash
cargo run -p screenroom-player --example capability            # libmpv on this device
cargo run --release -p screenroom-core   --example library_bench  # paging budget
cargo run --release -p screenroom-player --example render_bench   # offscreen frame budget
scripts/check-quit-routing.sh                                  # menu-bar Quit routing
scripts/verify-bundle.sh                                       # the packaged bundle
```

To install it as a real macOS application:

```bash
scripts/package-macos.sh                 # -> dist/Screenroom.app
scripts/package-macos.sh /Applications    # install a copy too
open dist/Screenroom.app
```

The bundle carries its own libmpv (verified to load from inside the bundle, not
just from Homebrew) but still resolves 20 Homebrew dependencies by absolute path,
and it is unsigned — correct for a personal install, not for distribution.
`scripts/verify-bundle.sh` checks the structure, the plist and every dependency,
and fails loudly when any of them is wrong.

The application opens the durable library at
`~/Library/Application Support/Screenroom/library.db`. Point it elsewhere with
`SCREENROOM_LIBRARY=/path/to/library.db`.

To grant a folder and scan it (no click-through needed yet for a scripted run):

```bash
cargo run -p screenroom-core --example prepare_library -- \
  "$HOME/Library/Application Support/Screenroom/library.db" "$PWD/.artifacts/media"
```

`ffmpeg` can generate rights-clear test media:

```bash
ffmpeg -y -f lavfi -i "testsrc2=size=1920x1080:rate=30:duration=60" \
       -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=60" -ac 2 \
       -c:v libx264 -pix_fmt yuv420p -preset veryfast -crf 28 -g 30 \
       -c:a aac -b:a 128k -shortest -movflags +faststart .artifacts/media/Test.mp4
```

`testsrc2` burns a timecode and frame counter into the picture, which is what makes
a screenshot usable as playback evidence.

## Design system integration

Private tokens are never committed. `apps/desktop/examples/export_ds_tokens.rs`
reads the pinned Dropout Design System and writes the Git-ignored
`apps/desktop/src/design/ds_pinned.rs`; `apps/desktop/build.rs` includes it when it
is present and otherwise uses the committed neutral fallback, so a clean checkout
still builds. The running app shows which pin produced its theme. See
[`docs/DROPOUT_DESIGN_INTEGRATION.md`](docs/DROPOUT_DESIGN_INTEGRATION.md).

## Read before coding

1. [`AGENTS.md`](AGENTS.md) — engineering rules and evidence standards.
2. [`docs/ADR-004_PERSONAL_EDITION_SCOPE.md`](docs/ADR-004_PERSONAL_EDITION_SCOPE.md)
   — the three targets, and what is out of scope.
3. [`docs/PRD.md`](docs/PRD.md) — product requirements F01–F25.
4. [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — how the pieces fit.
5. [`docs/IMPLEMENTATION_PLAN.md`](docs/IMPLEMENTATION_PLAN.md) — milestones.
6. [`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md) — what is
   done, verified, blocked.
7. [`docs/FEASIBILITY_GATES.md`](docs/FEASIBILITY_GATES.md) — device gates.
8. [`docs/TEST_PLAN.md`](docs/TEST_PLAN.md) — what each test layer proves.
9. [`docs/REUSE_LEDGER.md`](docs/REUSE_LEDGER.md), [`docs/DATA_MODEL.md`](docs/DATA_MODEL.md),
   [`docs/DESIGN.md`](docs/DESIGN.md), [`docs/LICENSING.md`](docs/LICENSING.md).

## Repository layout

```text
Cargo.toml                  # Rust workspace
crates/screenroom-core/     # domain, SQLite library, scanner, fingerprints
crates/screenroom-player/   # libmpv runtime boundary, player state machine
apps/desktop/               # iced + wgpu client, libmpv -> wgpu bridge
apps/tv/                    # Samsung Tizen client (not started)
scripts/                    # local evidence helpers (capture, drive), macOS packager
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
