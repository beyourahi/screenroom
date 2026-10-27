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
| G3b Audio output and audio/video sync | **NOT RUN** (not heard; no measured tolerance) |
| G4 Desktop playback quality on owner hardware | **BLOCKED** (no Windows hardware) |
| G5 Samsung Tizen physical device | **BLOCKED** (Tizen device and signing profile unavailable) |
| G6 Other TV platforms | **RETIRED** (LG webOS / Android TV out of scope, ADR-004) |
| G7 iOS | **RETIRED** (out of scope, ADR-003) |
| G8 Android TV + Media3 | **RETIRED** (out of scope, ADR-004) |
| G9 Durability, interruption and low-storage behaviour | **NOT RUN** |

---

## G1 — Toolchain and workspace

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all --check
```

Pass condition: all tests green, no first-party lint warnings, formatting clean.
Recorded: **106 tests, 0 failures**, `clippy -D warnings` clean, `fmt --check` clean
(2026-10-27, after the presentation fix and the M3 desktop UI work).

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
and release.** Audio and audio/video sync are tracked separately as G3b, because
nothing has been heard and no sync tolerance has been measured. The Windows D3D11
path is **BLOCKED** until that hardware exists, and is not inferred from macOS.
**Compiling is not passing.**

## G3b — Audio output and audio/video sync

The player initialises a CoreAudio client (`audio-client-name`, `video-sync=audio`)
and the process holds the CoreAudio/AudioDSP components. That is configuration
evidence only. The gate needs: (1) a human listening to a file with a known audio
track, and (2) a documented sync tolerance — how far audio may lead or lag video
before it is audible, and how that was measured.

Until both exist this gate is **NOT RUN**. It is the honest continuation of G3, not
a replacement for it.

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
deleted, and the recovery path is offered. Proven per platform or BLOCKED.

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
