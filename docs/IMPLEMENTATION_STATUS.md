# Implementation status ledger

Live record of what exists, what was actually verified, and what is blocked.
Future sessions resume from here instead of repeating completed work.

Last updated: **2026-10-27**, host **macOS ARM64 (Apple silicon), Command Line
Tools only, no full Xcode**.

**Scope (2026-10-26, [ADR-003](ADR-003_DESKTOP_AND_TV_SCOPE.md)):** desktop/laptop
and smart TVs only, with equal priority. iOS/Android phone and tablet apps, UniFFI
and the Flutter fallback are out of scope; none had been built, so no code was
removed. The development host's Flutter, flutter-tizen and .NET toolchains were
removed; the Flutter candidate stays archived on the `archive/*` branches.

---

## Preserved work (before any modification)

Three Codex worktrees contained uncommitted implementation. Nothing was deleted,
overwritten or moved; each was committed to a local archive branch so the tree
state is recoverable from Git as well as from the working directory.

| Worktree | Branch | Commit | Content |
| --- | --- | --- | --- |
| `.worktrees/full-app` | `archive/full-app-2026-10-25` | `15b3372` | Flutter app + packages + full docs, licensing, CI templates (149 files) |
| `.worktrees/source-check` | `archive/source-check-2026-10-25` | `fa6d145` | Flutter app + packages from the source/permission check pass (111 files) |
| `.worktrees/cache-controls` | `archive/cache-controls-2026-10-25` | `c1145aa` | Same plus artwork-cache work (113 files) |

Checks on those snapshots: no private Dropout tokens (`dropout_tokens.g.dart`),
no `vendor/dropout-design-system`, no `.env`/certificates/keystores and no user
media were committed. Build caches (`build/`, `.dart_tool/`, `.artifacts/`) were
excluded by the worktrees' own `.gitignore`. Largest committed blobs are the
pre-existing `reference/prototype-v0.1.zip` (573 KB) and a pre-existing test
fixture (127 KB).

`.worktrees/` remains Git-ignored in the root; the archive branches are the durable
copy. Do not delete either.

---

## M0 — Foundation and feasibility

**Status: host-verified.** Not device-verified.

| Action | Command | Result |
| --- | --- | --- |
| Workspace builds | `cargo check --workspace` | ok |
| Unit + integration tests | `cargo test --workspace` | 53 passed, 0 failed |
| Lints | `cargo clippy --workspace --all-targets` | no warnings from first-party code |
| Formatting | `cargo fmt --all --check` | clean |
| libmpv present | `cargo run -p screenroom-player --example capability` | `available: true`, `api version: 2.5`, `/opt/homebrew/lib/libmpv.2.dylib`, bridge `OpenGl` |
| libmpv decodes a real file | `cargo test -p screenroom-player --test libmpv_smoke -- --nocapture` | FILE_LOADED received, `duration` property returns a 2 s clip's duration |
| iced + wgpu shell compiles | `cargo check -p screenroom-desktop` | ok with iced 0.14.0 / iced_wgpu 0.14.0 / wgpu 27.0.1 |
| Video bridge compiles | `cargo check -p screenroom-desktop` | `video_surface.rs` type-checks: texture upload in `Primitive::prepare`, WGSL letterbox shader |

Toolchain observed: `rustc 1.99.1`, `cargo 1.99.1`, `ffmpeg/ffprobe 9.0.2`,
`node 24.20.0`, `python3 3.14.3`, `flutter 3.47.8` (only relevant to the archive;
removed from the host on 2026-10-26).

The clippy run prints one *dependency* warning: `block 0.1.6` will be rejected by
a future Rust version (transitive through the iced stack, not first-party code).

### What M0 does **not** prove

- No frame has been presented through wgpu from libmpv. The bridge compiles and
  the aspect logic is unit-tested; presentation on real hardware is M2.
- No GPU decode, 4K, HDR, audio output or subtitle rendering was exercised.
- Windows, Linux, Tizen, webOS and Android TV were not built.

---

## M1 — Shared core library

**Status: host-verified.** SQLite and filesystem behaviour tested on macOS only.

Implemented in `crates/screenroom-core`:

| Area | Public surface | Tests |
| --- | --- | --- |
| Sources | `add_source`, `sources`, `source`, `rename_source`, `relink_source`, `remove_source`, `set_source_state` | overlap rejection, invalid/relative roots, disconnect keeps history, re-grant restores identity |
| Profiles | `profiles`, `add_profile`, `rename_profile`, `remove_profile` | last-profile guard |
| Scanning | `LibraryScanner::scan`, `ScanToken` | media discovery, hidden dirs, sidecar/young files, cancellation, complete-rescan availability withdrawal, symlink refusal, single-scan lock, streaming updates |
| Ingestion | `ingest_file` (+`Ingest`) | idempotent rescan, replaced-file edition, small-file rename identity, unsafe locator rejection |
| Library | `items`, `count`, `item`, `continue_watching`, `seasons_for_work`, `known_genres`, `next_episode` | paging bounds, literal search escaping, work grouping, season filter/sort, gap-safe next episode |
| Playback | `start_session`, `begin_seek`, `checkpoint`, `end_session` | monotonic sequence, stale session, seek generation, impossible values, ended semantics |
| Lists | `set_favorite`, `set_watchlisted`, `mark_watched`, `hide_continue` | lists survive disconnect, Continue Watching ordering |
| Schema | `open`, `open_in_memory` | future schema fails closed without data loss; integrity check; crash normalisation |

Test totals: 13 unit (`filename`, `fingerprint`), 21 store (`tests/library.rs`),
7 scanner (`tests/scanner.rs`).

Design choice worth keeping: the Rust schema is byte-compatible with the layout the
Flutter candidate wrote (`user_version = 2`), so an existing on-device library opens
instead of being reset.

### Deliberately not ported yet

These exist in the archived Dart candidate and must be ported or explicitly
dropped before M3/M6 claims them:

| Dart API | Reason not ported now | Target |
| --- | --- | --- |
| `updateMetadata`, `updateEpisodeMetadata`, `clearMatch`, `setExternalIdentity`, `metadataRevision` | belongs with enrichment (M6), needs the provider client | M6 |
| `saveMarker`, `markers`, `removeMarker` | needs the player to validate intervals against duration | M2/M6 |
| `relinkCandidates`, `confirmFileRelink` | needs the relink UI flow | M3 |
| `exportBackup`, `importBackup` | validated versioned JSON format; port alongside the settings UI | M3 |
| `queueJob`, `jobs`, `updateJob` | store exists in schema; scheduling belongs with enrichment | M6 |
| Local NFO parsing | bounded parser; port with metadata ingestion | M6 |
| `recommendations` | explainable ranking over owned media | M6 |
| `authorizedPath` | platform adapter concern | M2 |

---

## M2 — Desktop playback slice

**Status: host-verified on macOS, including on-screen presentation.**
Everything below ran on this macOS host against libmpv 0.41 / client API 2.5.

| Area | What exists | Verification |
| --- | --- | --- |
| Player worker (`screenroom-player/src/worker.rs`) | One thread owns libmpv; commands in, immutable `PlayerSnapshot` out; lifecycle mapped onto `PlayerState` | `tests/worker.rs` — 5 tests |
| Software render bridge (`src/render.rs`) | mpv `sw` renderer → packed RGBA `Frame`, 64-byte aligned stride, bounded to 1920 px | real frames asserted non-blank with colour; `target_size`/stride unit tests; frame-rate assertion |
| Seek / pause / resume-at-load | `seek`, `set_paused`, `load_at(path, Some(seconds))` | position moves after seek, holds while paused, resumes after load |
| Failure states | `file_missing`, `load_failed`, `decode_failed`, `render_failed`, `seek_failed`, `pause_failed` with libmpv's own message | missing file test; end-file error path mapped |
| Durable resume (`apps/desktop/src/playback.rs`) | Session open → throttled checkpoints → seek generations → close; finished media resumes from the start | `tests/playback.rs` — 5 tests, including one that decodes a real clip, checkpoints, reopens the SQLite library and gets the same position back |
| Player screen (`apps/desktop/src/app.rs`) | Library list with progress, Back/Play-Pause/scrub/commit, resume label, failure text, Escape and Space keys | `clock` unit tests; open/refuse-to-open tests; window observation (below) |

Totals after M2: **106 workspace tests, 0 failures**; `clippy --workspace --all-targets
-- -D warnings` clean; `fmt --all --check` clean.

### On-screen presentation measured (2026-10-27)

```bash
cargo run -p screenroom-desktop          # window, signed-in desktops only, no browser
DB=~/Library/Application\ Support/screenroom/library.db
```

Observed, in the app's own window on this host, with the rights-cleared generated
fixture (not owner media):

| Observation | Evidence |
| --- | --- |
| A real decoded frame drawn through wgpu in the iced window | window-ID capture `34-playing-after-fix.png`), `.artifacts/evidence/` (Git-ignored) |
| Correct aspect: source pillarboxed inside a 3440×1440 window, no stretch, no colour inversion | same capture |
| Transport: play, pause, ±10 s seek, and end-of-file | `worker.rs` integration test; captures 24–30 |
| Resume across quit and relaunch: closed at 0:20, reopened playing at 0:26 with the burn-in timecode agreeing | capture `34-playing-after-fix.png`; `playback.rs` test decodes, checkpoints, reopens SQLite and reads the same position |
| **30.0 fps published, zero dropped frames, playhead 1.00×** at 1080p in a 3440×1440 window | `cargo test -p screenroom-player --test worker -- --nocapture` frame-rate assertion |
| Shipping configuration (`--release`) also measures **120 frames in 4.00 s = 30.0 fps at 1920×1080, playhead 1.00×** | `cargo test --release -p screenroom-player --test worker -- --nocapture` |
| Debug-build cost: idle ~3–5 % CPU, RSS plateau ~117 MB after playback ended (no leak in the sample); during software-bridge playback ~1 core / ~275 MB at 1080p | sampled with `ps`/`top` on a background run |
| Release-build cost at 1080p: playback steady at **~70 % of one core, RSS 254–255 MB flat across 12 s of samples** (no growth); after the clip ended, CPU fell to **~3 %** and RSS settled to **108 MB** | `ps -o %cpu=,rss=` against the release process |

### The frame-rate defect found and fixed here

Presentation initially measured **11.5 fps** against a 30 fps source, in a realtime
playhead (so frames were *dropped*, not merely slow). Instrumentation split the
cost: libmpv's `mpv_render_context_render` ≈ **3 ms**, the first-party `pack()` step
≈ **40 ms**. `pack()` copied pixel-by-pixel through a per-pixel `push`/alpha write in
the frame packer. Replacing the pixel loop with a chunked copy-and-stamp brought the
bridge to **30.0 fps with zero drops**. A separate speculative change to the worker's
wait slice was tried, measured to make no difference, and reverted — the fix is only
the pack path. The alpha channel is written as opaque during the pack rather than
assumed by the presentation shader.

### What M2 has **not** proven

- **Audio output** is initialised by libmpv (a CoreAudio client is opened,
  `video-sync=audio`) and the process holds the CoreAudio/AudioDSP components, but
  nothing has been **heard**.
- **Audio/video sync** has no measured tolerance (gate G3b).
- **Offline behaviour** was proven by holding zero network sockets during playback,
  not by disabling this machine's networking.
- **4K / hardware decode**: the software bridge is `hwdec=no` by design and capped
  at 1920 px, so 4K smoothness is neither claimed nor measured.
- **Windows**: nothing here ran on Windows, including the D3D11 path (unbuilt) and
  the case-insensitive path folding.

### Release-build measurement (2026-10-27)

```bash
cargo build --release -p screenroom-desktop
./target/release/screenroom-desktop      # same library, same fixture
cargo test --release -p screenroom-player --test worker -- --nocapture
```

| Metric | Release |
| --- | --- |
| Frame rate (software bridge, 1920×1080) | **30.0 fps, playhead 1.00×** |
| CPU during 1080p playback | ~66–76 % of one core |
| RSS during playback | 254–255 MB, flat across 12 s of samples |
| CPU after playback ended | ~3 % |
| RSS after playback ended | 108 MB |

The release figure is the one that ships. The software bridge is still a full-frame
CPU copy per frame by design (`hwdec=no`, bounded to 1920 px), so this cost is
expected and is the reason the zero-copy OpenGL/D3D11 paths remain the plan for 4K
and HDR rather than an optimisation of this path.

Two libmpv contract bugs were found and fixed while proving the above, and are
worth remembering: `MPV_RENDER_PARAM_SW_POINTER` takes the pixel buffer address
itself, and the render context must be freed before the mpv core or libmpv aborts
the process.

---

## M3 — Desktop core UI and design-system mapping

**Status: foundations delivered on macOS; not complete.**

Delivered: Home with Continue Watching and folder granting; a paged library grid
with search, kind and list filters, sorts and paging; the detail page with
resume/restart and the flag actions (watched, favourite, watchlist); keyboard
(arrows, Enter, Esc, ⌘1/⌘2, f/w/m) and mouse navigation; empty/loading/error
surfaces; and the Dropout Design System adapter with its private, commit-pinned,
Git-ignored token export (`apps/desktop/examples/export_ds_tokens.rs`).

Not delivered: next-episode advance, tracks and subtitles, profiles UI, settings,
backup/restore, and the 10,000-item budget run.

---

## Blocked gates

| Gate | Status | Reason |
| --- | --- | --- |
| Desktop frame presentation on real Windows GPU | BLOCKED | no Windows hardware/Visual Studio on this host |
| Windows 4K/HDR/hardware decode | BLOCKED | same |
| Physical Samsung UA50AU8000RXER (install, USB, AVPlay, durability) | BLOCKED | device and Tizen signing profile not available |
| Lightning 3 on the Tizen 6.0 engine | NOT YET RUN | needs the TV client and the device |
| Audible output and audio/video sync | NOT YET VERIFIED | not listened to; no tolerance measured |
| macOS application bundle signing | BLOCKED | Command Line Tools only, no full Xcode |

LG webOS and Android TV were retired by [ADR-004](ADR-004_PERSONAL_EDITION_SCOPE.md);
their former rows are kept in Git history, not as live work.

A blocked gate is not a pass. Do not mark any of these complete without a real
receipt.

---

## Open risks

1. **libmpv → wgpu bridge.** Compiles; presentation unproven. The software bridge
   costs one full-frame CPU copy per frame; 4K performance is unknown. Zero-copy
   D3D11 (Windows) / OpenGL (macOS) paths are designed but unbuilt.
2. **TV storage adapter.** Tizen may not allow SQLite; the fallback adapter and its
   durability are unproven on device.
3. **Lightning 3 frame budget** on a 2021 Tizen engine is unmeasured.
4. **Private design system boundary.** Mapping is implemented with the token export
   kept out of Git; a fully public build (no private pin at all) still has to be
   built once and proven to compile and run.
6. **Windows path semantics.** Case-insensitive overlap checks are implemented via
   `cfg!(windows)` folding but never executed on Windows.

---

## Next milestone

**M3, plus the named M2 gaps.** In order:

1. Close the remaining M2 honesty gaps that matter to the owner: confirm audio by
   ear and record an a/v-sync tolerance, and repeat the offline check with
   networking actually disabled (gate G3b).
2. Then continue M3: next-episode advance, tracks/subtitles, profiles, settings,
   backup/restore, and the 10,000-item budget run.

See [`IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md) for the full definitions of
done.
