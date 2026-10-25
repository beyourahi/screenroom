# Implementation status ledger

Live record of what exists, what was actually verified, and what is blocked.
Future sessions resume from here instead of repeating completed work.

Last updated: **2026-10-26**, host **macOS ARM64 (Apple silicon), Command Line
Tools only, no full Xcode**.

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
`node 24.20.0`, `python3 3.14.3`, `flutter 3.47.8` (only relevant to the archive).

The clippy run prints one *dependency* warning: `block 0.1.6` will be rejected by
a future Rust version (transitive through the iced stack, not first-party code).

### What M0 does **not** prove

- No frame has been presented through wgpu from libmpv. The bridge compiles and
  the aspect logic is unit-tested; presentation on real hardware is M2.
- No GPU decode, 4K, HDR, audio output or subtitle rendering was exercised.
- Windows, Linux, Tizen, webOS, iOS and Android were not built.

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

## Blocked gates

| Gate | Status | Reason |
| --- | --- | --- |
| Desktop frame presentation on real Windows GPU | BLOCKED | no Windows hardware/Visual Studio on this host |
| Windows 4K/HDR/hardware decode | BLOCKED | same |
| Physical Samsung UA50AU8000RXER (install, USB, AVPlay, durability) | BLOCKED | device and Tizen SDK/signing profile not available |
| iOS build + on-device AVPlayer | BLOCKED | no full Xcode on this host |
| Android build + Media3 + scoped storage | BLOCKED | no Android SDK/JDK-gradle setup verified on this host |
| UniFFI Swift/Kotlin binding generation | NOT YET RUN | planned as the first M4 step; documented support is verified, generation is not |
| Lightning 3 on the Tizen 6.0 engine | NOT YET RUN | needs the TV client and the device |
| macOS application bundle signing | BLOCKED | Command Line Tools only, no full Xcode |

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
4. **UniFFI ergonomics.** Large paged queries over the FFI boundary need a compact
   projection; not yet exercised.
5. **Private design system boundary.** A public build must not require it; the
   iced style mapping will sit behind an interface, and that interface does not
   exist yet.
6. **Windows path semantics.** Case-insensitive overlap checks are implemented via
   `cfg!(windows)` folding but never executed on Windows.

---

## Next milestone

**M2 — Desktop playback slice.** Sequence:

1. Player worker thread in `screenroom-player` (event loop, state transitions,
   frame generation, seek/end/error mapping).
2. Software render bridge wired to `video_surface.rs`; first real presented frame
   on this host.
3. Player screen with play/pause/seek/Back and truthful failure states.
4. Resume: session → checkpoint → relaunch restores position with the network off.
5. Record the new receipts here, then replace the M2 "next" status with evidence.

Entry condition: none — M1 is complete. Everything M2 needs is present on this
host (Rust toolchain, libmpv 0.41 / API 2.5, FFmpeg for synthetic media).
