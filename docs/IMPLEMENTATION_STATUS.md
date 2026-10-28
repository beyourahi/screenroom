# Implementation status ledger

Live record of what exists, what was actually verified, and what is blocked.
Future sessions resume from here instead of repeating completed work.

Last updated: **2026-10-28**, host **macOS ARM64 (Apple silicon), Command Line
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
| Preferences | `preferences`, `save_preferences` | defaults, survive a reopen, BCP-47 validation rejects an injected tag and saves nothing, unknown keys preserved |
| Backup | `export_backup`, `inspect_backup`, `import_backup` | every table round-trips into a fresh library, the restore stamp is stripped, the previous library is kept beside the new one, refuses to overwrite the live database, refuses a non-Screenroom SQLite file and a newer format, a failed backup leaves no partial file |
| Schema | `open`, `open_in_memory` | future schema fails closed without data loss; integrity check; crash normalisation |
| Recovery | reopening after a crash | an open session is closed, a running job is requeued, progress survives the whole recovery sequence, a damaged database is reported rather than replaced |

Test totals: 38 unit (`filename`, `fingerprint`, `artwork`, `markers`,
`recommend`, library migration), 27 store (`tests/library.rs`), 12 scanner
(`tests/scanner.rs`), 19 storage (`tests/storage.rs`), 7 recovery
(`tests/recovery.rs`), 2 scale (`tests/scale.rs`).

### 10,000-item budget run (2026-10-28)

```bash
cargo run --release -p screenroom-core --example library_bench
cargo test -p screenroom-core --test scale
```

Generated 10,000-entry library, release build, page size 24:

| Operation | Time |
| --- | --- |
| Ingest 10,000 entries | 825 ms (12,118 entries/s) |
| First page | 18.4 ms |
| **Last page** (worst case for an OFFSET scan) | **43.5 ms** |
| Added ordering | 34.4 ms |
| Year ordering | 16.6 ms |
| Progress ordering | 15.5 ms |
| Literal search (1 hit / no hit) | 16.4 ms / 16.6 ms |
| `count` | 14–18 ms |

The slowest paged query is 43.5 ms, well inside a frame at 30 fps for a UI that
pages rather than scans. This is the M3 budget run for the store layer; it measures
queries, not rendering.

#### Repeat run and scaling (2026-10-28, later the same day)

Re-run on the same machine to check the numbers above were not a single lucky
sample, and extended to larger libraries to find where it stops feeling instant:

| Library | First page | Last page | `count` | Literal search |
| --- | --- | --- | --- | --- |
| 100 | 0.37 ms | 0.36 ms | 0.11 ms | 0.21 ms |
| 1,000 | 0.75 ms | 1.38 ms | 0.44 ms | 0.57 ms |
| **10,000** | **6.4 ms** | **15.3 ms** | **5.3 ms** | **6.1 ms** |
| 50,000 | 48.3 ms | 146.1 ms | 44.7 ms | 71.5 ms |

Two things this settles, and one it does not:

- The cost is genuinely **proportional to library size**, not a fixed overhead: it
  rises ~17× from 1,000 to 10,000 and ~8× again from 10,000 to 50,000 entries. An
  earlier reading of an identical ~10 ms across every query suggested a constant
  setup cost; measuring across sizes showed that was sampling noise, not the shape.
- **10,000 entries is comfortably inside a frame.** A page plus its count — what
  the Browse screen runs per keystroke — is ~11.7 ms against 16.7 ms at 60 fps.
- **50,000 is not**, at ~93 ms together, so a very large personal library would need
  keyset paging instead of `OFFSET`. That is recorded as a risk rather than fixed
  ahead of a library that size existing.

Run-to-run variance is real and is not hidden: the first ledger run measured
18.4 ms / 43.5 ms for first/last page where this one measured 6.4 ms / 15.3 ms on
the same machine and input. Both are recorded; neither supersedes the other. Any
future comparison should re-run `library_bench` rather than quote a memory of it.

#### The catalogue facet stall, and schema 4 (2026-10-29)

The paging numbers above were never the whole story: opening Browse on a 10,000-entry
library was unusable, and it was not `items` or `count` that did it. `known_genres`
runs on every refresh, before the screen draws a card, and its `EXISTS` subquery
joined `editions` on `work_id` — a column with no index. Measured with
the same generated library, release build:

| `known_genres` on 10,000 entries | Time |
| --- | --- |
| Before the indexes (schema 3) | **44.2 s** |
| After the indexes (schema 4) | **13.7 ms** |

That is the class of failure the loose bounds in `tests/scale.rs` exist to catch, and
it was found by using the app on a large library rather than by the benchmark: the
bench did not ask the catalogue for its facets at all. It does now — `library_bench`
times `known_genres` and `continue_watching` next to paging, which is what put the
44-second figure in the table above instead of a memory of it.

Schema 4 is that fix: indexes on the join columns every catalogue query walks
(`editions_work`, `files_edition`, `file_episodes_episode`, `markers_edition`). It is
additive like schema 3 — no table or row is rewritten — so a schema 2 or 3 library is
migrated in place, in one transaction, and a schema 2 library takes both steps at
once. Both paths are exercised: `tests/library.rs` opens a schema 2 library, and a
unit test in `library/mod.rs` opens a schema 3 library and asserts the four indexes
exist and the profile survives. The ingest cost of maintaining them is real but
small: 10,000 entries in 394 ms with the indexes against 440 ms without in one
comparable pair of runs.

### The real library on a USB SSD (2026-10-29)

Everything above was measured on generated fixtures. This is the owner's actual
movie and TV library on an exFAT USB drive, read-only, scanned into a **separate
test database** (`SCREENROOM_LIBRARY`), with the real library's checksum taken before
and after and unchanged. The drive was never written to: no thumbnails, databases,
logs or temporary files, and no mount, permission or repair operation.

**Inventory, built from the mounted filesystem rather than from the earlier tree
snapshot:** 830 media files (729 `.mkv`, 101 `.mp4`), 941.1 GiB, mean 1,161 MiB;
19 show folders, 291 films; 539 files with an `SxxEyy` name; 21 sidecar `.srt`; two
filenames with non-ASCII characters, 20 with apostrophes, 3 with colons; two files at
depth 6 inside a `Duology` folder. `AppleDouble` `._*` companions, `Auto Data Backup`
and the hidden directories were excluded from the inventory by rule.

**Scan (release build, metadata only — no media content read):**

| Run | Visited | Imported | Issues | Wall | Peak RSS |
| --- | --- | --- | --- | --- | --- |
| First scan | 1,002 | 830 | 0 | 3.59 s | 8.2 MB |
| Rescan of the same library | 1,002 | 830 | 0 | 2.31 s | — |

Resulting catalogue: 830 files, 291 films, 18 series, 53 seasons, 539 episodes,
539 file↔episode links, **zero** files whose stored season/episode disagrees with
its episode row, and 830 editions after the rescan (no duplicates). Every episode
count per show matches an independent count of the folder: 149/149 The Mentalist,
72/72 Suits, 40/40 From, 30/30 Jack Ryan, 26/26 What If, 26/26 House of the Dragon,
and so on for all 18.

`scripts/probe-library.sh` then read every file's container, codecs and streams with
ffprobe (bounded to four concurrent reads, output outside the media root):

| | Count |
| --- | --- |
| Probed | 830 |
| Readable | 827 |
| **Unreadable (truncated MP4, `moov atom not found`)** | **3** |
| Video — HEVC Main 10 / HEVC Main | 396 / 251 |
| Video — H.264 High / Main / High 10 | 119 / 16 / 1 |
| Video — AV1 Main 10 / MPEG-4 Simple | 43 / 1 |
| Files with embedded subtitle streams | 646 (3,785 SubRip, 375 PGS, 210 ASS, 164 DVD, 9 mov_text) |
| Audio tracks per file — 1 / 2 / 3 / 4 | 659 / 160 / 7 / 1 |
| Audio codecs | AAC, AC-3, E-AC-3, Opus, DTS, Vorbis, MP3 |
| Anamorphic (SAR ≠ 1:1) / ≥ 3840 px wide | 8 / 8 |
| Files with embedded cover art | 115 (68 JPEG, 47 PNG) |

The three unreadable files are the Cloverfield trilogy. They are catalogued (the
scanner sees a file and records it) and the player reports the failure honestly:
`decode_failed — unrecognized file format`. They are not silently skipped, and the
scan is not treated as incomplete because of them.

**Playback, offscreen, through the real worker** (`examples/play_file.rs`, muted,
four-second windows):

| File | Codec / container | First frame | Delivery | Audio clock | Seek |
| --- | --- | --- | --- | --- | --- |
| Superman (2025) | HEVC 10-bit, PGS + 7 SubRip | 4.5 s | 23.7 fps, worst gap 51 ms | 4.01 s / 4.00 s | 88 ms |
| Avengers Infinity War | HEVC 4K → 1920×800 target | 2.1 s | 24.0 fps | 3.96 s / 4.00 s | 116 ms |
| Suits S07E02 | AV1 10-bit, Opus 5.1 | 0.4 s | 24.0 fps | 3.86 s / 4.00 s | 90 ms |
| Tulsa King S01E01 | HEVC, external `.srt` picked up | 1.5 s | 24.0 fps | 4.00 s / 4.00 s | 88 ms |
| The Shining | H.264 High 10, DTS 5.1 | 3.6 s | 24.0 fps | 3.00 s / 3.00 s | — |
| Predators | H.264, 4 audio tracks | 0.2 s | 24.3 fps | 3.00 s / 3.00 s | — |
| The Hitman's Bodyguard | MPEG-4 Simple, 1280×536 | 1.4 s | 24.0 fps | 3.02 s / 3.00 s | — |
| Terrifier (2016) | H.264 High, AC-3 | 2.2 s | 23.7 fps | 4.00 s / 4.00 s | 76 ms |
| Cloverfield (2008) | broken MP4 | **failed** — `decode_failed` | — | — | — |

Audio track switching (to the last of four) and subtitle `sub-scale`/`sub-pos`
read-back were confirmed on every file that has them, and external sidecar SRTs are
loaded by libmpv itself and appear as an `external=true` subtitle track. Anamorphic
content arrives letterboxed inside the frame (Tulsa King: 1920×1080 with 60 px bars),
not stretched.

**Posters.** The hidden-window application ran against the test database and wrote
21 JPEG posters (400 px wide, 180 KB total) for the works on screen, from the real
files through ffmpeg. Sampled by hand, one is a real frame rather than a card.

#### Defects found by the real library, and fixed

| Symptom | Cause | Fix |
| --- | --- | --- |
| 21 extra entries with titles like `._12 Angry Men - 1957`, and 3 phantom episodes | macOS AppleDouble companions keep the media extension; the walk skipped hidden **directories** but not hidden **files** | skip dot-files in the walk, silently (`scanner.rs`); regression test `apple_double_companions_are_not_media` |
| All 18 series had a null year | the year only came from the episode filename, and episode names never carry it — the show folder does | an episodic file with no year of its own takes the show folder's (`filename.rs`); a film in a collection folder does not, because `Duology (2009-2011)` is a range, not a release year. Test: two `The Office` shows at different years stay two works |
| A cover-art JPEG appeared as a selectable **video** track | `track-list` reports embedded artwork as video; it was listed like a second angle | skip video tracks with `image=yes` (`worker.rs`); `Predators` now lists one video track, not two |
| Only the first 16 posters of a 24-card page were ever extracted; the rest stayed blank until a key was pressed | the queue is 16 deep and nothing refilled it when an extraction finished | refill the freed slot on the next poll, for works on screen only — not a library-wide marathon (`app.rs`) |

Measured after the fixes: 830 imported (was 851), 18 series with years (was 0), one
video track on a file with cover art (was two), 21 posters for the visible works
(was 16).

#### What the real library still does not do

- **No stored durations.** `editions.duration_ms` is `0` for all 830 files until each
  one is played, so the catalogue cannot show runtimes or a progress bar for an
  unwatched film, and `mark_segment` answers `duration_unknown` before first play.
  Deliberate so far (the player reports the truth when it opens the file), but on a
  real library it is visible as a missing runtime on every card.
- **Posters are film frames, not cover art.** 115 files carry an embedded cover
  image which would be a better poster for those; the extractor samples 10% into
  the video instead.
- **Anamorphic content is letterboxed twice**: the frame already carries 60 px bars,
  and the window fits the 1920×1080 frame as 16:9, so a 2:1 film is drawn smaller
  than it needs to be. Not stretched, just not optimal.
- **Hardware decoding is still off** (`hwdec=no` by design for the CPU bridge), which
  is why a 4K HEVC film takes ~2 s to first frame and decodes in software.
- **On-screen behaviour is unverified.** Every number above is offscreen: no window,
  no GPU surface, nothing heard. Resume-after-quit, full-screen, the D-pad-free
  keyboard journey and the on-screen frame budget still need a permissioned session.

Design choice worth keeping: the Rust schema is byte-compatible with the layout the
Flutter candidate wrote (`user_version = 2`), so an existing on-device library opens
and is migrated forward to the current version instead of being reset.

### Deliberately not ported yet

These exist in the archived Dart candidate and must be ported or explicitly
dropped before M3/M6 claims them:

| Dart API | Reason not ported now | Target |
| --- | --- | --- |
| `updateMetadata`, `updateEpisodeMetadata`, `clearMatch`, `setExternalIdentity`, `metadataRevision` | belongs with enrichment (M6), needs the provider client | M6 |
| ~~`saveMarker`, `markers`, `removeMarker`~~ | **ported 2026-10-29** as `set_marker`/`mark_segment`/`markers`/`remove_marker`; intervals are validated against the edition's stored duration | done |
| `relinkCandidates`, `confirmFileRelink` | needs the relink UI flow | M3 |
| `exportBackup`, `importBackup` | validated versioned JSON format; port alongside the settings UI | M3 |
| `queueJob`, `jobs`, `updateJob` | store exists in schema; scheduling belongs with enrichment | M6 |
| Local NFO parsing | bounded parser; port with metadata ingestion | M6 |
| ~~`recommendations`~~ | **ported 2026-10-29** as `Library::recommendations`, explainable over owned media only | done |
| `authorizedPath` | platform adapter concern | M2 |

---

## M2 — Desktop playback slice

**Status: host-verified on macOS, including on-screen presentation.**
Everything below ran on this macOS host against libmpv 0.41 / client API 2.5.

| Area | What exists | Verification |
| --- | --- | --- |
| Player worker (`screenroom-player/src/worker.rs`) | One thread owns libmpv; commands in, immutable `PlayerSnapshot` out; lifecycle mapped onto `PlayerState` | `tests/worker.rs` — 10 tests |
| Software render bridge (`src/render.rs`) | mpv `sw` renderer → packed RGBA `Frame`, 64-byte aligned stride, bounded to 1920 px | real frames asserted non-blank with colour; `target_size`/stride unit tests; frame-rate assertion |
| Seek / pause / resume-at-load | `seek`, `set_paused`, `load_at(path, Some(seconds))` | position moves after seek, holds while paused, resumes after load |
| Audio and A/V sync | `audio-pts`, mpv's own `avsync` and `total-avsync-change` in `PlayerSnapshot` | source volume measured, the audio clock advances in real time (0.982× wall), libmpv `avsync` 0.0 ms with a 12 ms cumulative adjustment |
| Tracks | track list with language/title/selected read from libmpv; `set_audio_track`, `set_subtitle_track` | two audio and two subtitle tracks enumerated and switched; selection read back rather than assumed |
| Failure states | `file_missing`, `load_failed`, `decode_failed`, `render_failed`, `seek_failed`, `pause_failed` with libmpv's own message | missing file test; malformed file reported rather than shown as an empty player; end-file error path mapped |
| Durable resume (`apps/desktop/src/playback.rs`) | Session open → throttled checkpoints → seek generations → close; finished media resumes from the start | `tests/playback.rs` — 6 tests, including one that decodes a real clip, checkpoints, reopens the SQLite library and gets the same position back |
| Player screen (`apps/desktop/src/app.rs`) | Library list with progress, Back/Play-Pause/scrub/commit, resume label, failure text, Escape and Space keys, audio/subtitle pickers, next-episode autoplay | `clock` unit tests; open/refuse-to-open tests; window observation (below) |

Totals after M2: **106 workspace tests, 0 failures**; `clippy --workspace --all-targets
-- -D warnings` clean; `fmt --all --check` clean.
### Audio and sync measured (2026-10-28)

Hearing a file was never going to be repeatable, so the player now reads libmpv's
own instrumentation and a test asserts on it. Measured on this host, generated
fixture with a known audio track:

| Measurement | Observed |
| --- | --- |
| Source has real audio | `volumedetect` mean −24.1 dB |
| Audio clock over a 9 s window | 8.87 s of audio against 9.13 s of video, **0.982× wall clock** |
| libmpv `avsync` | **0.0 ms**, cumulative adjustment 12 ms |
| Mistimed frames | none reported |
| Audio output buffer vs video playhead | −262 ms, moving −267 ms across the window |

That last row is the one worth remembering: `audio-pts` is a **buffer position,
not a lip-sync indicator**. It drifts to −262 ms on a file that libmpv reports as
perfectly in sync, so asserting on it would have been a permanent false failure.
The sync assertion uses `avsync`; the buffer gap is printed as an observation only.

This does not close gate G3b on its own: nobody has yet listened to output through
speakers. It does turn "audio probably works" into a measured 0.982× real-time audio
clock and a 0.0 ms sync error, which is the evidence a listening pass would confirm.

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

- **Audible output** has not been *heard*: the audio clock, volume and `avsync` are
  now measured (above), which is instrument evidence rather than a listening test.
- **Audio/video sync** has no measured tolerance (gate G3b).
- **Offline behaviour** was proven by holding zero network sockets during playback,
  not by disabling this machine's networking.
- **4K / hardware decode**: the software bridge is `hwdec=no` by design and capped
  at 1920 px, so 4K smoothness is neither claimed nor measured.
- **Windows**: nothing here ran on Windows, including the D3D11 path (unbuilt) and
  the case-insensitive path folding. `WINDOWS_VALIDATION.md` is the handoff that
  turns each of these into a runnable check.

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

**Status: the essential desktop surface is delivered and exercised on macOS; a few
M3 items are still absent, and they are named below rather than implied.**

Delivered and observable in the running app:

| Area | What exists |
| --- | --- |
| Home | Continue Watching first, then recent items; folder granting; scan status |
| Library | Paged grid (24/page) with search, kind and list filters, four sorts and paging |
| Detail | Work/season structure, editions, progress, resume/restart, watched, favourite, watchlist |
| Player | Play/pause, ±10 s seek, scrub, resume label, **audio and subtitle pickers** (with a real “Off” for subtitles), failure copy |
| Next episode | Autoplay driven by the stored preference and the core's gap-safe ordering; a gap stops the chain instead of skipping it; an unreadable next episode is refused with its reason |
| Keyboard | Arrows, Enter, Esc, ⌘1/⌘2/⌘3 (Home/Browse/Settings), f/w/m, Space, ←/→ seek in the player |
| Settings | Viewer profiles with per-profile progress, lists and Continue Watching; autoplay and language preferences; folders with availability, distinct recovery copy, rescan, relink and remove; backup and restore |
| Design | Dropout adapter with the private, commit-pinned, Git-ignored token export (`apps/desktop/examples/export_ds_tokens.rs`); the running app names the pin that produced its theme |

Still absent from M3: previews, and the **on-screen** rendering/input budget. The
store half is done (see M1) and the offscreen half is measured below; what remains
is the iced window itself, which needs a visible window and is therefore pending.

### Subtitle styling delivered (2026-10-28)

Size and vertical position are now preferences, applied as libmpv's own `sub-scale`
and `sub-pos`. Both are stored as whole percentages and offered as sliders in
Settings (50–300% size, 0–100% position, where 100% is the bottom of the picture).

The player **reads both properties back** after setting them rather than echoing the
request, so `PlayerSnapshot::subtitle_scale`/`subtitle_position` are evidence of what
libmpv accepted. Verified against a real clip:

| Check | Result |
| --- | --- |
| Style reaches libmpv (1.75×, 88%) | read back as requested, within 1e-2 |
| Style survives a file change | unchanged across `load` |
| Impossible style (−5.0, 9000) | clamped to `SUB_SCALE_RANGE.0` / `SUB_POSITION_RANGE.1` |
| Non-finite style (`NaN`, `inf`) | skipped; last good value stands; player not failed |
| Stored value outside range | refused on write (`subtitle_scale_out_of_range`) |
| Unusable stored value | clamped on read; the library still opens |

Measured along the way: libmpv holds `sub-scale` in **single precision**, so 0.1
reads back as `0.10000000149`. That is why the interface shows a whole percentage and
why the assertions are tolerances rather than equalities.

Honest limitation, carried into the UI copy: a subtitle script that positions its own
lines — a styled ASS release — keeps that positioning, because Screenroom leaves
`sub-ass-override` alone. Size still applies; placement may not.

### Offscreen render and command latency measured (2026-10-28)

A window was previously the only way to answer “can this machine keep a 1080p file
moving”. `crates/screenroom-player/examples/render_bench.rs` answers it with no
window at all: it builds its own moving fixture with FFmpeg, drives the real decode →
libmpv render → CPU frame path, and counts **distinct** frames by libmpv's monotonic
`generation` so a repeated snapshot cannot inflate the rate.

```bash
cargo run --release -p screenroom-player --example render_bench
cargo run --release -p screenroom-player --example render_bench -- 1920x1080 12
```

MacBook Air M4, release build, libmpv 2.5 (OpenGL bridge, software frame target):

| Source | Time to first frame | Sustained delivery | Worst frame gap |
| --- | --- | --- | --- |
| 1280×720 | 283 ms | 24.0 fps of a 24 fps source | 47.7 ms |
| **1920×1080** | **95–278 ms** | **24.0–24.1 fps of a 24 fps source** | **47.9–54.0 ms** |
| 3840×2160 | 132 ms | 24.1 fps | 53.8 ms |

| Command | Round trip to the playhead arriving |
| --- | --- |
| 1080p seek to 2.0 s / 5.0 s / 1.0 s | 72.7 ms / 60.0 ms / 58.7 ms |
| 4K-source seek (see caveat) | 62.3 ms / 111.7 ms / 205.6 ms |

`avsync` held at **+0.0000 s** across every run, and the playhead stayed at 1.00×
wall clock (9.04 s of playhead in 9.00 s of wall).

Three things this does **not** prove, stated because the numbers look better than
they are:

- **It is not an on-screen frame rate.** iced, the wgpu upload and the compositor
  add cost this measurement cannot see. These are a ceiling on what the display can
  reach, not a substitute for measuring the window. The on-screen budget at 10,000
  items is recorded as **PENDING** in `docs/TEST_PLAN.md`.
- **The 4K row is not 4K presentation.** `render::MAX_DIMENSION` is 1920 and the
  software bridge downscales larger sources, so the 4K fixture was decoded at 4K but
  **rendered at 1920×1080** — the run reports `frame size: 1920x1080`. Native 4K
  presentation is unmeasured and stays behind gate G2.
- **Time to first frame has a wide spread** (95 ms to 278 ms across runs on the same
  input). It is the least stable number here and should not be quoted to three
  significant figures.

The seek figures are also worth reading carefully: an earlier version of this example
reported **sub-microsecond** seek latency, which was a broken measurement — the check
was “playhead is past the target”, and a playhead already 11 s in satisfied it before
the seek took effect. The condition is now “arrived at the target *and* moved”, which
a backwards seek cannot satisfy early. The first number was wrong and is recorded here
so the correction is visible rather than silently replaced.

Two behaviours were corrected while exercising the finished surface, both from
watching the real app rather than the tests:

- The subtitle picker was inert when a file had no subtitle tracks. It now says
  “none in this file” and is not offered as a control that does nothing.
- “resumed from 0:33” stayed on screen next to “Ended”. Resume copy is now only
  shown while there is something to resume.

### Closing the window now ends the viewing (2026-10-28)

A quit used to leave the session row `active` and lose up to a second of progress.
The cause was that neither ⌘Q nor the window close box reached the update loop at
all — the app exited underneath it. iced's `window::close_requests()` subscription
is the hook that fires before teardown, and it now runs the same graceful close as
Back: final checkpoint, then `end_session`.

Measured in the packaged app against the real database:

| Exit path | Final progress | Session |
| --- | --- | --- |
| Window close box | **52.0 s, exact** | `active = 0` |
| ⌘Q (**after** routing, below) | **as the close box: both paths identical** | `active = 0` |

⌘Q initially lost under a second and left the row `active`: the Quit item AppKit
installs sends `terminate:`, which ends the process *below* iced, so no close request
is ever delivered and nobody finalises the viewing. The crash-recovery normalisation
from M1 covered it — the next launch was clean rather than stuck — but "recovered on
the next launch" is not "stored".

`apps/desktop/src/quit.rs` now finds the application menu's Quit item **by selector,
never by title** (the title is localised and carries the app's name), and re-points it
at `performClose:` — the exact action the close button performs. The menu item and the
close button are therefore one operation and cannot disagree about whether the open
file was stored. A menu whose Quit item cannot be identified is left untouched, because
a half-rewritten menu bar is worse than the behaviour it replaces.

The first attempt at this reported `Routed` and still exited via `terminate:`: the
guard treated the initial `Unsupported` value as "already graceful", so routing never
ran. Both the routing and the two dismissal paths were then re-verified, and
`scripts/verify-quit.sh` keeps it reproducible.

**Verification status of this one, stated plainly:** the two dismissal paths were
confirmed with a visible window and real key presses, which is now gated behind
`SCREENROOM_ALLOW_FOREGROUND=1` (2026-10-28 policy). The evidence stands as recorded,
but it has **not** been re-run under the background-first regime. `scripts/check-quit-routing.sh`
re-checks the routing half headlessly, with a hidden window, on every run.

## macOS application bundle (2026-10-28)

```bash
scripts/package-macos.sh                  # -> dist/Screenroom.app
scripts/package-macos.sh /Applications     # install into a folder as well
```

The bundle carries its own copy of libmpv in `Contents/Frameworks`, and the player
looks there after the system paths (`packaged_candidates` in
`crates/screenroom-player`), so an installed app keeps working when the Homebrew
prefix changes or a development copy is removed.

| Observation | Result |
| --- | --- |
| Bundle assembles, `Info.plist` valid, Gatekeeper quarantine cleared for a locally built copy | yes |
| Runs **outside cargo**, from `dist/Screenroom.app` | yes — library, playback, Settings all exercised |
| libmpv loaded from the bundle, client API 2.5 | yes |
| Private DS pin in the bundled theme | `dropout-design-system@c000425 semantic-dark` |
| Progress written while playing | yes, 10.4 s → 16.8 s in the live database |

**What the bundle does not do:** libmpv's own dependencies (FFmpeg, libass,
libplacebo) still resolve by absolute Homebrew install names, so the app runs on a
machine with the same `mpv` installed. Vendoring FFmpeg is a licensing decision
(FFmpeg's build options decide which codecs may be redistributed) and is not
implied by a personal install; the script reports the dependency list instead of
hiding it. There is **no signing and no notarization**, which is correct for a
personally built app and is recorded as a blocked gate for distribution.

#### Runtime dependencies, named (2026-10-28)

Rebuilt and re-verified headlessly with `scripts/verify-bundle.sh`, which takes no
focus and opens nothing. The bundle is 17 MB and the binary is arm64.

The bundled `libmpv.2.dylib` (4.3 MB, a **real file** — `cp -L` follows Homebrew's
symlink so an in-place upgrade cannot dangle it) declares **20 absolute
dependencies**. All 20 must exist on the machine that runs the app:

| Dependency | Homebrew formula |
| --- | --- |
| `libmpv.2.dylib` | `mpv` |
| `libavcodec.63`, `libavfilter.12`, `libavformat.63`, `libavutil.61`, `libswresample.7`, `libswscale.10`, `libavdevice.63` | `ffmpeg` |
| `libass.9` | `libass` |
| `libplacebo.387` | `libplacebo` |
| `libmujs` | `mujs` |
| `liblcms2.2` | `little-cms2` |
| `libarchive.13` | `libarchive` |
| `libbluray.4` | `libbluray` |
| `libluajit-5.1.2` | `luajit` |
| `librubberband.3` | `rubberband` |
| `libuchardet.0` | `uchardet` |
| `libzimg.2` | `zimg` |
| `libjpeg.8` | `jpeg-turbo` |
| `libvulkan.1` | `vulkan-loader` |

All 20 resolved on this host. `brew install mpv` pulls the whole set, which is why
the packaging script can check them and report before anything is installed.

`scripts/verify-bundle.sh` checks, and each one can actually fail:

| Check | Result |
| --- | --- |
| Structure: `Info.plist` and an executable `Contents/MacOS/Screenroom` | pass |
| `Info.plist` parses; `CFBundleExecutable` and `CFBundleIdentifier` are set | pass (`com.screenroom.desktop`) |
| Bundled libmpv is a real file, not a symlink | pass |
| Bundled libmpv is a Mach-O arm64 library | pass |
| Every absolute dependency resolves | pass (20/20) |
| **libmpv from inside the bundle loads and creates a core** | pass — API 2.5, path reported as the bundle's own |

The last row is the one worth having. `capability()` prefers the system paths, so on
a development machine it always finds Homebrew's copy and never touches the bundle's
— which is exactly the copy that has to work on a machine *without* Homebrew. The
check therefore names the path explicitly (`capability_at`), which is what makes it
evidence about the bundle rather than about the host.

Negative tests were run too, because a check that cannot fail proves nothing:

| Input | Exit | Result |
| --- | --- | --- |
| No bundle at the given path | `2` | refuses to guess |
| A bundle with no executable and an unparsable plist | `1` | reports 3 failed checks |
| `/bin/ls` named as libmpv | `1` | `does not export mpv_create: dlsym failed` |
| The real bundle | `0` | all checks pass |

**Still not proven, by design:** that a frame reaches the screen, or that the window
opens at all. Both need a visible window and an owner-approved session.

## M4 — Samsung Tizen client (`apps/tv`), 2026-10-28

The TV client is built. It is a **new application**, not a port: the television does
not run the Rust core, so the client speaks the core's entities and invariants over its
own storage adapter. What was written, and what each part rests on:

| Piece | What it is |
| --- | --- |
| `src/core/` | the contract ported from `crates/screenroom-core/src/domain.rs`, plus the rules that decide things: the D-pad focus machine (the desktop's rule, exactly), the session/generation/sequence progress guard, the durable-document store, relative-locator validation, the scanner's rules (bounded walk, identity across rescans, withdrawal only after a complete walk), filename interpretation, the track menu and decoder preferences, and the verified next-episode rule |
| `src/platform/` | the Tizen boundary: `webapis.avplay` behind a playback port, remote-key registration, `localStorage` behind the storage port, removable-storage enumeration, the bounded cancellable walk, and `tizen.filesystem.toURI` for the decoder's address. The host probe reports what it found rather than assuming |
| `src/design/` | the same semantic tokens the desktop uses, multiplied for viewing distance, and the card/grid model the screens draw from |
| `src/screens/` | Home, Browse, Detail, Settings and Player, each owning one focus zone |
| `config.xml`, `vite.config.ts`, `scripts/prepare-tizen-dist.mjs` | the widget manifest, the engine-targeted bundle, and the step that turns the bundle into an installable widget directory |

### Verified here (2026-10-28)

```bash
cd apps/tv
pnpm typecheck    # clean; application source sees no ambient Node types at all
pnpm test         # 189 tests, 0 failures
pnpm build        # 484,867-byte main.js + the staging step
```

**2026-10-28, later: the screen was actually looked at, and it was broken.** The build
installed on the set drew one line of text on black. Reproduced in a browser at
1920×1080 against the *built* widget, which is what made it debuggable at all:

| Defect | Cause | Fix |
| --- | --- | --- |
| Only static text painted; every value set in `ready()` stayed empty | Blits makes an attribute reactive only when it starts with `:` (`isReactiveKey = (str) => str.startsWith(':')` in its code generator), so `content="$subheading"` was read once, at creation | every state-bound attribute now carries `:` |
| Home, Browse and Detail placed nodes at `undefined` | their templates read geometry members off a *computed object* as if they were flat state | the geometry is copied into state by `applyLayout()` before the first paint, the way Settings and Player already did |
| A card's title was drawn past the card's bottom edge | the title strip was guessed (`body * 2.4`) and shorter than the two lines under the art | `gridFor` derives the strip from the type scale and exposes `titleY`/`subtitleY` |
| Browse's third row was drawn below the set | the grid was sized from the width alone | `gridFor` takes the band it has to fit and shrinks the cards; Browse pages 5×2 |
| A file that would not open said so into an invisible node | the whole player template's alpha was the overlay's alpha, which starts at 0 | the scrim is its own layer; failures go through `reportFailure`, which keeps the message up |

Guarded by `test/templates.test.ts` (no template may read a name the component does not
declare) and the new `test/cards.test.ts` case (a card fits its own text; a page fits its
band). **Verified in the browser against the built widget only — not yet on the set.**

The signed package was produced through the `tizen-build-project` runner:
`apps/tv/build/ScreenroomTV-0.1.0.wgt`, 157,202 bytes, sha256
`b7d82c49755ae1501bffe62040cd2263aaa181510d477d3c4196d8b5710ecc51`. Its envelope
carried **no** "signed with Tizen default developer certificates" warning, and the
`main.js` inside the package is byte-identical to the tested bundle (sha256
`9b7cbdaea4a9bc2b5b3b3855d6382593742a751032c8153b3c917176bae597c4`). One signing
profile exists and is active (`screenroom`); no certificate material was inspected.

### What the client does now, on and off the device

- **The library is populated by the TV itself.** Settings → *Scan for media* lists the
  platform's storages, walks every mounted removable one with a bounded, cancellable
  walk, parses film and episode identity from filenames, and writes the catalogue, the
  sources and the scan report in **one** document. A drive that is not mounted, or that
  will not list, withdraws nothing.
- **Progress survives a pulled cable.** Item ids are carried forward per path across
  rescans, so a stored position stays attached; a drive that leaves marks its rows
  unreadable and never deletes them, and the storage-state listener does that the
  moment the platform reports the change.
- **Playback has a track menu.** Up/Down opens audio tracks, *Subtitles off* and the
  subtitle tracks; a choice is remembered as this device's preference and applied when
  the next file becomes ready. `setSilentSubtitle` is what "off" means, because a track
  index cannot express it.
- **A sidecar subtitle file is used where the decoder takes one** (`.smi`, `.srt`,
  `.sub`, `.ttxt` beside the video, the four Samsung documents). The app draws the text
  the decoder hands back, and says so when the decoder refuses the file.
- **A finished episode offers the next one** only when the catalogue holds exactly one
  candidate one number up in the same season.
- **The remote actually reaches the screens.** Every screen declares its keys in an
  `input` block — the first build declared none, so arrow keys, Enter and Back landed
  nowhere — and the player claims Back so the track menu closes before the file does.

### What M4 does **not** prove

- **Nothing has run on a Tizen engine.** No frame rendered, no WebGL probe, no frame
  budget. A packaged `.wgt` is a file, not a device result.
- **No media has reached AVPlay.** No URI, codec, seek or track switch. The URI form is
  now the platform's own (`toURI`), which removes a guess but not the need to try it.
- **No drive has been walked by a television.** The walk is exercised against a
  simulated Tizen filesystem (`test/scan-pipeline.test.ts`), which proves the wiring and
  proves nothing about USB permissions or timing on the set.
- **The TV's DUID has not been checked against the distributor certificate**, which is
  what decides whether the package installs at all.
- **The television could not be reached on 2026-10-28.** A network sweep of
  `192.168.0.0/24` on SDB port 26101 found no device, `sdb devices` listed nothing, and
  the saved-remote list was empty — so the earlier session's connection is not
  recoverable from here and the next step needs the set switched on and in developer
  mode.

Gate G5 is **PARTIAL** — see `TIZEN_DEVICE_HANDOFF.md` for what the set itself reported; the checklist and the questions only the
TV can settle are in [`TIZEN_DEVICE_HANDOFF.md`](TIZEN_DEVICE_HANDOFF.md).

**2026-10-29 — the set was reachable again and the fixed build went on it.** Package
`01d60176…d2231`, 160,680 bytes, `install completed`, launched, and the TV's own record
reads `canvases=1 size=1920x1080 css=1920px/1080px client=1920x1080 viewport=1920x1080
dpr=1 mounted=1` — the blank screen is gone at the engine level. The owner then reported
the **home screen with readable text and cards**, so item 1 now passes on both halves.
Items 3–12 (drive, scan, playback, tracks, subtitles, next episode, focus, restart,
reinstall) remain open and need the remote in a hand: this firmware accepts no synthetic
key press, exposes no screenshot and no `dlog` over TCP.

### Test totals after the M3 work (2026-10-28)

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all --check
```

**189 tests, 0 failures**, clippy with no first-party warnings, formatting clean:

| Suite | Tests |
| --- | --- |
| `screenroom-core` unit (filename, fingerprint, path identity) | 23 |
| `core/tests/library.rs` (incl. per-viewer isolation, path semantics) | 27 |
| `core/tests/storage.rs` (preferences, subtitle style, backup) | 19 |
| `core/tests/scanner.rs` (incl. an unplugged drive, a deleted folder, a withdrawn grant) | 10 |
| `core/tests/recovery.rs` | 7 |
| `core/tests/scale.rs` (10,000 items) | 2 |
| `screenroom-player` unit (incl. the bundled-libmpv search order) | 20 |
| `player/tests/worker.rs` (real decode, audio, tracks, subtitle style) | 12 |
| `player/tests/libmpv_smoke.rs` | 2 |
| `screenroom-desktop` unit (incl. the window-width grid rule) | 69 |
| `desktop/tests/playback.rs` | 6 |
| `desktop/tests/artwork.rs` | 3 |

Workspace total after M3b: **211 passed, 0 failed**.

The count is not the coverage. Two scanner tests have `#[cfg(unix)]` bodies, and the
fourteen player tests print `skipped: …` and pass vacuously when no libmpv is found.
Neither of those is a Windows-specific gap; both are recorded so the number is not
mistaken for evidence it does not carry.

Headless scripts, which are not part of `cargo test`:

| Script | What it proves |
| --- | --- |
| `scripts/check-quit-routing.sh` | the running app re-pointed the menu bar's Quit — hidden window, no focus taken |
| `scripts/verify-bundle.sh` | the packaged bundle's structure, plist, bundled libmpv and all 20 of its dependencies |
| `scripts/foreground-guard.sh` | sourced by every foreground script; refuses without `SCREENROOM_ALLOW_FOREGROUND=1` |

---

## M3b — retained posters, skip segments, local suggestions (2026-10-29)

**Status: host-verified on macOS.** Three gaps that made the desktop feel unfinished
are closed, and each one is closed without a network, a model or a guess.

| Capability | Store (`screenroom-core`) | Desktop (`apps/desktop`) |
| --- | --- | --- |
| Offline artwork (F06) | new `artwork` table (schema **3**), `store_artwork`, `artwork`, `artworks_for`, `artwork_candidates`, `artwork_candidates_in`, `missing_artwork_count`. JPEG only, ≤768 KiB, ≤4096 px per side, magic number checked, one poster per work, replacing is an overwrite | `src/artwork.rs` runs ffmpeg as a child process (`-nostdin`, arguments as separate values so a filename can never be a shell fragment), one frame per work, 20 s deadline per run, output capped, fallback to the first frame when a stored duration seeks past the end; a bounded 16-deep queue on its own thread, never while a file is playing. Cards, Detail and Home draw the retained bytes |
| Skip intro/credits (F14) | `set_marker`, `mark_segment` (duration taken from the edition, `duration_unknown` when it is still unknown), `markers`, `remove_marker`; one marker per kind per edition; an interval that does not fit the film is refused, not clamped | Player pins a segment at the playhead (`i` intro, `c` credits, again to end it; `s` skips), offers *Skip intro*/*Skip credits* only while the playhead is inside a stored marker, and `auto_skip_intro` (default **off**) skips once per viewing |
| Local recommendations (F16) | `Library::recommendations`: next episode of a series in progress or just finished, then the viewer's watchlist, then more of a genre already being watched, then recently added. One row per work, unavailable and watched files excluded, each row carries its reason | Home draws a *Suggested for you* shelf from the same cursor order as Continue watching, with the reason on each card |

Schema change is additive and migrated: a `user_version = 2` library gains the
`artwork` table in place and keeps every row (asserted by a test that builds a
schema-2 database and opens it). A backup written before the table existed restores
with no artwork rather than failing — the restore reads only the tables the file
actually holds.

### Evidence from the running application

Four generated clips (three films, one series episode) were scanned into a real
library, then the application was launched against it with a hidden window and no
foreground:

```bash
cargo run -p screenroom-core --example prepare_library -- /tmp/sr-art/library.db /tmp/sr-art/media Library
SCREENROOM_LIBRARY=/tmp/sr-art/library.db SCREENROOM_WINDOW=hidden ./target/debug/screenroom-desktop
sqlite3 /tmp/sr-art/library.db "SELECT w.title, a.width, a.height, a.sampled_at_ms, LENGTH(a.bytes) FROM artwork a JOIN works w ON w.id = a.work_id;"
```

Observed: four rows, each `400×226`, `8,883` bytes — one poster per work, produced by
the running application from real ffmpeg runs, not by a test fixture. `sampled_at_ms`
is `0` for all four because a 4-second clip has no stored duration, so the first probe
is at 120 s; the fallback to the first frame is what turned that into a poster rather
than a failure, which is the behaviour that test covers.

Checks: `cargo test --workspace` **211 passed, 0 failed**; `clippy --workspace
--all-targets` clean apart from the pre-existing transitive `block 0.1.6`
future-incompat note; `fmt --all --check` clean.

### What this does **not** prove

- Posters have been **stored and handed to the widget**, and the mapping is unit
tested, but nobody has looked at a visible window since they were added. Drawing is
**PENDING** an owner-approved foreground session, like the on-screen budget.
- No real personal media was sampled; the posters above come from generated test clips.
- The skip button has not been pressed during a real viewing; its interval rule, the
once-only auto-skip and the store's validation are tested.
- Recommendations are exercised against synthetic libraries; how they read with a
real 1,200-file library is unmeasured.

---

## Blocked gates

| Gate | Status | Reason |
| --- | --- | --- |
| Desktop frame presentation on real Windows GPU | BLOCKED | no Windows hardware/Visual Studio on this host; every `cfg!(windows)` branch is unbuilt as well as unrun. The exact handoff — commands, expectations, acceptance checks and named falsifications — is `WINDOWS_VALIDATION.md` |
| Windows 4K/HDR/hardware decode | BLOCKED | same |
| Physical Samsung UA50AU8000RXER — install | **PASS** (2026-10-28) | connected at `192.168.0.172:26101`; `sdb capability` reports `vendor_name:Samsung`, `profile_name:tv`, `platform_version:6.0`, `cpu_arch:armv7`. The distributor chain is Samsung-issued with exactly one `deviceid` binding, and the DUID the set reports matches it (compared by hash; neither value printed). `vd_appinstall` reached `install completed` and `vd_applist` lists `PlYByk3psw.ScreenroomTV` 0.1.0 |
| Physical Samsung UA50AU8000RXER — playback, USB, durability | NOT YET RUN | needs a person with the remote: this firmware accepts no synthetic key press, exposes no screenshot, no `dlog` over TCP and no inspector port. Items 3–7 of G5 |
| Lightning 3 on the Tizen 6.0 engine | **PASS on the engine, PARTIAL on measurement** (2026-10-29) | the set's own record: `host=tizen avplay=1 filesystem=1 inputdevice=1 storage=1 keys=9/refused=0 model=UAU8000 firmware=T-NKLAUABC-2310.0` and `canvases=1 size=1920x1080 css=1920px/1080px client=1920x1080 viewport=1920x1080 dpr=1 mounted=1`, no `error` or `rejection` entries, engine `76.0.3809.146`. The bundle executes, Lightning mounts a full-size surface, and the **owner has seen the home screen with readable text and cards**. WebGL capability and frame pacing are still not measured |
| Audible output heard by a person | NOT YET VERIFIED | the audio clock and `avsync` are measured; nobody has listened |
| macOS bundle signing and notarization for distribution | BLOCKED | Command Line Tools only, no full Xcode; not required for a personal install |

LG webOS and Android TV were retired by [ADR-004](ADR-004_PERSONAL_EDITION_SCOPE.md);
their former rows are kept in Git history, not as live work.

A blocked gate is not a pass. Do not mark any of these complete without a real
receipt.

---

## Open risks

1. **libmpv → wgpu bridge.** Presentation is proven on macOS at 1080p. The software
   bridge costs one full-frame CPU copy per frame, so 4K performance is unknown and
   expected to be insufficient. Zero-copy D3D11 (Windows) / OpenGL (macOS) paths are
   designed but unbuilt.
2. **TV storage adapter.** Tizen may not allow SQLite; the fallback adapter and its
   durability are unproven on device.
3. **Lightning 3 frame budget** on a 2021 Tizen engine is unmeasured.
4. **Private design system boundary.** Mapping is implemented with the token export
   kept out of Git; a fully public build (no private pin at all) still has to be
   built once and proven to compile and run.
5. **Windows path semantics — the rules are closed, the platform is not.** The
   folding rule was a `cfg!` and is now a value (`PathSemantics`, chosen by
   [`Library::with_path_semantics`]): case-folding is a property of the *volume*, not
   of the operating system, and a case-sensitive APFS or NTFS volume exists. Both
   branches are tested from this Mac, and the key rule is asserted directly
   (`/Media/Shows` folds to `/media/shows` under `CaseInsensitive` and is left alone
   under `CaseSensitive`).

   The *spelling* rules were added on 2026-10-28 (`paths::location_key`, with
   `PathStyle` as a value for the same reason), because comparing paths literally
   was wrong on Windows in two ways: `std::fs::canonicalize` returns a verbatim path
   there, so the scanner's symlink guard refused **every** healthy Windows source as
   a replaced link, and `D:\Media` did not compare equal to `D:/Media`, so the same
   folder could be granted twice. Both are asserted from this Mac with
   Windows-shaped inputs.

   **Still open:** the file-system half. On this host's case-folding volume,
   `Media/SHOWS` resolves to `Media/Shows`, so the case-sensitive *grant* path cannot
   be reached through the file system here; the test detects that and asserts the
   reachable half instead of claiming the other. And **no Windows target compiles
   from this Mac at all**: `cargo check --target x86_64-pc-windows-msvc` fails inside
   `libsqlite3-sys`, whose bundled SQLite needs the MSVC SDK's headers
   (`fatal error: 'stdlib.h' file not found`). That is a toolchain gap, not a code
   failure, and it means every `cfg!(windows)` branch in the tree — the libmpv
   candidate names, the D3D11 bridge choice, the `WindowsPortable` folder logic — is
   still unbuilt and unrun. Windows remains a blocked gate; `WINDOWS_VALIDATION.md`
   is the handoff.

6. **Packaged binary vs development tree.** The `.app` bundles libmpv but not its
   transitive Homebrew dependencies, and it is unsigned. That is deliberate for a
   personal install; making it portable is a separate licensing and signing task.
7. ~~**⌘Q is ungraceful.**~~ **Fixed** — `apps/desktop/src/quit.rs` re-points the
   menu bar's Quit at `performClose:`, so the menu item and the close box are one
   operation. The two dismissal paths were confirmed identical with real key presses;
   that confirmation predates the background-first policy and has not been re-run
   under it, while `scripts/check-quit-routing.sh` re-checks the routing headlessly.
8. **Keyset paging for very large libraries.** The query budget is inside a frame at
   10,000 entries but not at 50,000 (~93 ms for a page plus count), where `OFFSET`
   must be replaced with keyset paging. Not fixed ahead of a library that size
   existing; measured in M1 above.
9. **A drive that returns under a different letter needs a relink.** Windows mints
   a new drive letter for the same volume, and the letter is part of the path
   spelling, so a USB source granted as `E:\Media` is not recognised as `F:\Media`:
   there is no volume-serial identity yet. The recovery is an explicit relink, which
   re-issues the grant and re-proves every file, so nothing is deleted and history,
   lists and resume positions survive. Recorded as a limit rather than a defect
   because the recovery path exists and is the honest one — that two letters are one
   volume is not something a string can know.
10. **A download sidecar in capitals is not noticed on a case-sensitive file
    system.** `has_download_sidecar` probes for the lowercase suffix with `exists()`,
    which finds `Movie.mkv.CRDOWNLOAD` on Windows and on this host's case-folding APFS
    volume but not on Linux. There the file is imported while still being written and
    is caught by the size/mtime settle check instead. Left alone because the
    Windows-side case — the one this session had to answer — already works, and the
    Linux half means matching case against the directory listing the walk already
    holds.

---

## Next milestone

**Finish M3's remainder, then the two honesty gaps that only the owner can close.**

1. Remaining M3: browse previews, and the **on-screen** rendering/input budget at
   10,000 items. The M3b posters also need one visible-window pass to be called
   drawn rather than stored. Subtitle styling is delivered (above); the store and offscreen
   halves of the budget are measured (M1 and the render section above). The on-screen
   half needs a visible window, so it is **PENDING** an owner-approved session.
2. Listening pass for gate G3b: hear a known audio track, and confirm the measured
   0.0 ms `avsync` figure matches what is heard.
3. Repeat the offline check with networking actually disabled rather than by
   observing zero sockets.
4. Native 4K presentation. The offscreen 4K row above measures a **1920×1080**
   render target because `render::MAX_DIMENSION` caps it, so gate G2's 4K half is
   untouched by it.

See [`IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md) for the full definitions of
done.
