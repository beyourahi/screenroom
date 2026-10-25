# Feasibility gates

> **2026-10-26:** rewritten for the Rust-first architecture
> ([ADR-002](ADR-002_RUST_FIRST_STACK.md)). Gates are per capability and per
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
| G3 libmpv frame presented through wgpu in an iced window | **IN PROGRESS** (bridge compiles; no frame presented) |
| G4 Desktop playback quality on owner hardware | **BLOCKED** (no Windows hardware) |
| G5 Samsung Tizen physical device | **BLOCKED** (device/SDK/signing unavailable) |
| G6 LG webOS / other TV | **NOT RUN** |
| G7 iOS UniFFI + AVPlayer | **NOT RUN** (no full Xcode) |
| G8 Android UniFFI + Media3 | **NOT RUN** (no verified SDK setup) |
| G9 Durability, interruption and low-storage behaviour | **NOT RUN** |

---

## G1 — Toolchain and workspace

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all --check
```

Pass condition: all tests green, no first-party lint warnings, formatting clean.
Recorded: 53 tests, 0 failures, 2026-10-26.

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
   `Primitive::prepare` → draw. Compile: **done**. Present a decoded frame:
   **not yet**.
2. Measure on this host at 1080p: CPU time per frame, dropped frames, memory.
3. macOS: OpenGL render context → IOSurface/readback → wgpu. Evaluate only if the
   software bridge cannot keep up or hardware decoding requires it.
4. Windows: D3D11 texture → wgpu shared import. Required for 4K/HDR parity.

Pass condition: ≥ 30 fps sustained on the target device with correct aspect, no
tearing or colour inversion, and audio/video sync within a documented tolerance.
Recorded measurement or BLOCKED. **Compiling is not passing.**

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

## G6 — Other TV platforms

LG webOS: packaged `.ipk`, native player, storage durability. Android TV/Google TV
belongs to G8. Each additional platform gets its own row in the capability matrix
and its own device receipt.

## G7 — iOS

- UniFFI generates Swift bindings that compile in the app target.
- The app grants access to files/folders and keeps the grant across launches.
- AVPlayer plays a local file, resumes at the stored position, and honours
  audio/subtitle selection where the OS supports it.
- Backgrounding during playback checkpoints progress.
- PiP (if claimed) uses the real OS PiP, not a fake overlay.

## G8 — Android

- UniFFI generates Kotlin bindings that compile with the JNA runtime packaged.
- SAF tree grants are persisted and canonicalized; the core's root rules hold.
- Media3 plays a local file; a foreground service keeps playback alive correctly.
- TV variant: D-pad focus, leanback behaviour, and no touch-only affordance.
- Scoped-storage edge cases (revoked grant, removed SD card) produce the truthful
  availability states, not crashes.

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
