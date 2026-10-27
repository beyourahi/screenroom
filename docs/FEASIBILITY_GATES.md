# Feasibility gates

> **2026-10-26:** rewritten for the Rust-first architecture
> ([ADR-002](ADR-002_RUST_FIRST_STACK.md)) and the desktop + TV scope
> ([ADR-003](ADR-003_DESKTOP_AND_TV_SCOPE.md)). Gates are per capability and per
> device; a framework label never satisfies one.

A gate is **PASSED** only with a first-hand receipt recorded in
[`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md): command, device model,
firmware/OS, package hash where relevant, and the observed behaviour. Otherwise it
is **NOT RUN**, **IN PROGRESS** or **BLOCKED**. A compile, a unit test, a simulator,
a browser tab or a mock never closes a gate.

Current summary:

| Gate | State |
| --- | --- |
| G1 Rust workspace builds and tests on the dev host | **PASSED** (macOS ARM64) |
| G2 libmpv client API (load, command, event, property) | **PASSED** (macOS ARM64, libmpv API 2.5) |
| G3 libmpv frame presented through wgpu in an iced window | **PASSED on macOS** (30 fps, correct aspect, transport and resume verified in the window); Windows path **BLOCKED** |
| G3b Audio output and audio/video sync | **PARTIAL** — audio clock and `avsync` measured (0.982× real time, 0.0 ms); not heard by a person |
| G4 Desktop playback quality on owner hardware | **BLOCKED** (no Windows hardware) |
| G5 Samsung Tizen physical device | **BLOCKED** (Tizen device and signing profile unavailable) |
| G6 Other TV platforms | **RETIRED** (LG webOS / Android TV out of scope, ADR-004) |
| G7 iOS | **RETIRED** (out of scope, ADR-003) |
| G8 Android TV + Media3 | **RETIRED** (out of scope, ADR-004) |
| G9 Durability, interruption and low-storage behaviour | **PARTIAL on the host** (crash-state and damaged-database recovery tested); no device run, no power cut, no full disk |

---

## G1 — Toolchain and workspace

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all --check
```

Pass condition: all tests green, no first-party lint warnings, formatting clean.
Recorded: **149 tests, 0 failures**, `clippy -D warnings` clean, `fmt --check` clean
(2026-10-28, after the tracks/audio, settings/backup, recovery, scale and packaging
work). The suite includes real libmpv decode, audio and track switching, a 10,000-item
query budget, and SQLite crash/damage recovery.

### Local installation (supporting check, not a gate)

```bash
scripts/package-macos.sh
open dist/Screenroom.app
```

The bundle was built, launched outside cargo and driven through library, playback
and Settings with its own bundled libmpv. This proves the app is an installed local
application rather than a development harness. It does **not** prove distribution:
the bundle is unsigned and its libmpv still resolves Homebrew dependencies by
absolute paths. Signing is **BLOCKED** (Command Line Tools only, no Xcode) and is
not required for a personal install.

## G2 — libmpv client API

```bash
cargo run -p screenroom-player --example capability
cargo test -p screenroom-player --test libmpv_smoke -- --nocapture
```

Pass condition: libmpv is found at runtime, a core is created and destroyed
cleanly, a real file reaches `FILE_LOADED`, and a property read returns a plausible
value.

This gate covers the *client API*. It says nothing about video output.

## G3 — The desktop presentation bridge

The question: can a libmpv frame reach an iced window through wgpu at an
acceptable rate with correct colour and aspect?

Steps:
1. Software bridge: mpv `sw` renderer → RGBA buffer → wgpu texture upload in
   `Primitive::prepare` → draw.
   * decode → correctly packed RGBA frame, with real pixels: **done**
     (`crates/screenroom-player/tests/worker.rs`).
   * upload → draw inside a running window: **done on macOS (2026-10-27)** — the
     app's own window shows the decoded frame with correct pillarbox aspect, moving
     frames, transport and resume. Captured by window ID (never a screen region) so
     no other application on the display can be recorded.

   Two contract details found while proving this, both now encoded in the code:
   `MPV_RENDER_PARAM_SW_POINTER` takes the surface address itself (a
   pointer-to-pointer corrupts the caller's stack), and the render context must be
   released *before* the mpv core or libmpv aborts.
2. Measured on this host at 1080p in a 3440×1440 window: **30.0 fps published,
   zero dropped frames, playhead 1.00×**; libmpv render ≈ 3 ms/frame and the
   first-party pixel pack, once fixed, no longer dominates. Debug build: idle
   ~3–5 % CPU and ~117 MB RSS; software-bridge playback ~1 core / ~275 MB. Release
   build (the one that ships): **30.0 fps at 1920×1080**, ~66–76 % of one core and
   flat ~255 MB RSS during playback, falling to ~3 % CPU / 108 MB after it ends.
3. macOS: OpenGL render context → IOSurface/readback → wgpu. Evaluate only if the
   software bridge cannot keep up or hardware decoding requires it.
4. Windows: D3D11 texture → wgpu shared import. Required for 4K/HDR parity.

Pass condition: ≥ 30 fps sustained on the target device with correct aspect, no
tearing or colour inversion, and audio/video sync within a documented tolerance.
**macOS software bridge: 30.0 fps at 1080p with correct aspect — met, in both debug
and release.** Audio and audio/video sync are tracked separately as G3b: sync is now
measured (0.0 ms `avsync`) but nothing has been heard. The Windows D3D11
path is **BLOCKED** until that hardware exists, and is not inferred from macOS.
**Compiling is not passing.**

## G3b — Audio output and audio/video sync

Two halves, and only one is done.

**Measured (2026-10-28), by a test rather than by assumption:**

```bash
cargo test -p screenroom-player --test worker audio_is_consumed -- --nocapture
```

- The fixture really has audio (`volumedetect` mean −24.1 dB).
- The audio clock advanced 8.87 s against 9.13 s of video in a 9 s wall window:
  **0.982× real time**, i.e. audio is being consumed, not stalled.
- libmpv `avsync` **0.0 ms** with a 12 ms cumulative adjustment and no mistimed
  frames. The assertion fails above 100 ms.
- `audio-pts` is **not** a sync figure: it drifts to −262 ms on the same file, so it
  is printed as an observation and never asserted on.

**Still open:** nobody has *listened*. The gate needs a person hearing a known audio
track through speakers, and a documented tolerance for what is audible. Instrument
numbers are not a listening test, so G3b stays **PARTIAL**, not passed.

## G4 — Desktop playback quality (owner hardware)

On the owner's Windows machine: hardware decode for H.264/HEVC/AV1 where the GPU
supports it, MKV/MP4/M2TS containers, multiple audio tracks, internal and external
subtitles, 10-bit and HDR signalling, 4K, ultrawide window behaviour, fullscreen and
DPI changes, device switching mid-playback, and a 30-minute soak without leaks.

Until that machine is available: **BLOCKED**. Do not infer it from macOS results.

## G5 — Samsung Tizen (physical UA50AU8000RXER)

1. `flutter`-era assumptions are void; verify the Tizen 6.0 web engine actually runs
   a Lightning 3 bundle: WebGL available, frame pacing, canvas fallback not needed.
2. Signed `.wgt` installs through the developer profile; certificate material stays
   outside Git.
3. USB/removable storage is enumerable and readable through the platform API, with
   the TV's own permission prompt understood.
4. AVPlay opens a local file, seeks, reports duration, and handles pause/resume.
5. D-pad focus is reachable, visible at couch distance, and Back restores the
   previous focus.
6. Durable state and artwork bytes survive app restart and a TV power cycle.
7. Document what a reinstall destroys, and that external backup is required.

Each numbered item is its own receipt. Guessing at any of them is prohibited.

## G6 — Other TV platforms (retired)

LG webOS and Android TV were retired on 2026-10-27 by
[ADR-004](ADR-004_PERSONAL_EDITION_SCOPE.md): the owner's television is a Samsung
UA50AU8000RXER and the personal edition targets that set plus desktop. Apple TV and
Roku were never in scope. The number is kept so old receipts stay unambiguous.

## G7 — iOS (retired)

Retired on 2026-10-26 by [ADR-003](ADR-003_DESKTOP_AND_TV_SCOPE.md): phone and
tablet apps are out of scope. The number is kept so old receipts stay unambiguous.

## G8 — Android TV / Google TV (retired)

Retired on 2026-10-27 by [ADR-004](ADR-004_PERSONAL_EDITION_SCOPE.md). Kept as a
number so old receipts stay unambiguous; no Android TV work is planned.

## G9 — Durability and interruption

On each real device: kill the process during playback, cut power, unplug the media
drive, fill the disk, revoke permission, and corrupt the database copy. Expected:
the last bounded checkpoint survives, the library remains browsable, nothing is
deleted, and the recovery path is offered.

**Host coverage (2026-10-28), `core/tests/recovery.rs` — 7 tests:** a session left
`active` by a killed process is closed on reopen, a job left `running` is requeued,
progress survives the whole sequence, an unsupported or future schema is refused
without resetting data, a damaged database is reported rather than replaced, and an
empty file is initialised.

**Additional host observation:** closing the app's window runs the graceful path and
stores the exact position (52.0 s, session closed); ⌘Q terminates below the event
loop, losing under a second, and the next launch's recovery pass closes the row.

**Still open:** power loss, a pulled USB drive, a full disk and a revoked grant on
real hardware. Those are device runs, not database tests, and this stays PARTIAL.

---

## How to record a gate

Append to [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md):

```text
G<number> <capability> — <device, OS/firmware, build hash>
command or steps
observed: <exact result, numbers where relevant>
verdict: PASSED | FAILED | BLOCKED (reason)
```

Never edit a verdict upward without a new run. Never convert "compiles" into
"works", "simulator" into "device", or "looks right" into "measured".
