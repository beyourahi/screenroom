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
| G5 Samsung Tizen physical device | **PARTIAL** — the signed package installs on the owner's set, the bundle executes there, and the TV itself reports a mounted 1920×1080 canvas; AVPlay is present; no drive has been walked and no frame, sound or remote press has been observed yet |
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
Recorded: **189 tests, 0 failures**, `clippy -D warnings` clean, `fmt --check` clean
(2026-10-28, after the tracks/audio, settings/backup, subtitle styling, recovery,
scale, path-semantics and packaging work). The suite includes real libmpv decode,
audio and track switching, subtitle style read-back, a 10,000-item query budget,
per-viewer isolation, an unplugged drive, and SQLite crash/damage recovery.

Supporting checks in the same tier, all windowless and all runnable on demand:
`scripts/check-quit-routing.sh`, `scripts/verify-bundle.sh`,
`cargo run --example library_bench` and `cargo run --example render_bench`. The
offscreen frame budget they produce is a **ceiling**, not an on-screen frame rate;
gate G2's on-screen half remains open.

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

### What is settled without the device (2026-10-28)

Item 2 now passes on the television, and item 1 passes in its mechanism (the engine runs
the bundle and mounts a correctly sized canvas) while its visual half is still open.
Items 3–7 are untouched. What a Mac could answer, it answered:

| Question the gate depends on | State |
| --- | --- |
| Does a Lightning 3 bundle build for this engine at all? | **Yes.** `pnpm build` emits a single 462,142-byte `main.js` targeted at the 2021 engine, with no code splitting — a widget has no server to fetch a second chunk from — and relative asset paths, verified by the staging step rather than trusted |
| Is the client's own logic exercised? | **161 tests, 0 failures**, off-device and in `pnpm test`: the D-pad rule index-for-index against the desktop's, the progress guard, the store's durability rules, locator validation, AVPlay's display-rect maths and track reading, the scan rules and a simulated-Tizen walk of a whole drive, the track menu and next-episode rules, decoder preferences, the remote-key wiring of every screen, tokens and grid geometry |
| Does the widget manifest ask for the right things? | **The privileges are the ones the code uses** — remote-key and filesystem-read, and the template's unused `application.launch` request is gone. The TV is still the only authority on whether it grants them, and a refused `listDirectory` is reported verbatim on the Settings screen rather than shown as an empty drive |
| Can a signed package be produced? | **Yes.** `build/ScreenroomTV-0.1.0.wgt`, 157,202 bytes, sha256 `b7d82c49755ae1501bffe62040cd2263aaa181510d477d3c4196d8b5710ecc51`, produced by the `tizen-build-project` runner with **no** default-developer-certificate warning, and containing a `main.js` byte-identical to the tested bundle (`9b7cbda…97c4`) |
| Is the TV authorised to install it? | **Yes, verified without printing either value.** The distributor chain is Samsung-issued (`CN=VD DEVELOPER Public CA Class` ← `VD DEVELOPER Public Root Class`), leaf valid 2026-10-27 → 2027-10-27, carrying exactly **one** `deviceid` binding; the DUID the set reports through `cert-manager acquire-duid` matches that binding (compared by hash, 2026-10-28) |
| Can the television be reached? | **Yes.** Connected at `192.168.0.172:26101`; `sdb capability` reports `vendor_name:Samsung`, `profile_name:tv`, `platform_version:6.0`, `cpu_arch:armv7` |

### What has actually run on the television (2026-10-28)

Measurements below are the television's own, read back with `sdb pull` — not the Mac's.
The client writes them into its own `localStorage` because this firmware exposes no
screenshot, no `dlog` over the network, no inspector port and no key injection, so
otherwise an installed, launched app is indistinguishable from a working one.

| What | Receipt |
| --- | --- |
| The signed `.wgt` installs | `vd_appinstall` reached `install completed`; `vd_applist` then lists `PlYByk3psw.ScreenroomTV` 0.1.0, 157,986 bytes — the packaged size, to the byte |
| The app launches | `was_execute` → `launched`; the TV's own `use_count` increments per launch |
| **The bundle executes** | the set wrote its own `boot` record: `host=tizen avplay=1 filesystem=1 inputdevice=1 storage=1 keys=9/refused=0 model=UAU8000 firmware=T-NKLAUABC-2310.0 product=3.7` |
| **Lightning mounts a render surface** | the set wrote `canvases=1 size=1920x1080 mounted=1`, and **no** `error` or `rejection` entries |
| The engine | `Mozilla/5.0 (SMART-TV; LINUX; Tizen 6.0) … 76.0.3809.146/6.0 TV` — Chromium 76, which is why the bundle targets `es2019` |
| The remote's extra keys | registration accepted **9 of 9** media/Info keys and refused none |
| AVPlay is present | `avplay=1` after the namespace fix below |

**Not yet observed on the set, and not claimed:** a single frame seen by a person,
frame pacing or WebGL capability measured, a drive walked, any file played, a seek, a
track change, a subtitle, a resume, focus at couch distance, or survival of a power
cycle. Those are items 1 (visual part), 3, 4, 5, 6 and 7, and they need a person with
the remote — the checklist in [`TIZEN_DEVICE_HANDOFF.md`](TIZEN_DEVICE_HANDOFF.md) is
written for exactly that, with the observable that settles each one.

### Three defects only the television could find (fixed, 2026-10-28)

1. **The widget loaded its bundle as a module and drew nothing.** Vite emitted
   `<script type="module" crossorigin src="./main.js">`, and a widget runs from a
   `file://` origin where Chromium refuses module scripts on CORS grounds. Proven both
   ways in the same origin class: the module build yields `0` canvases with
   `blocked by CORS policy` on `main.js`, the fixed build mounts a 1920×1080 canvas. The
   staged page now loads `<script defer src="./main.js">`, `import.meta.url` is
   compiled away, and the staging step **fails the build** if a module script or
   module-only syntax survives.
2. **AVPlay was looked for in the wrong namespace.** The client read
   `tizen.webapis.avplay`; `webapis` is a **sibling** of `tizen` on `window`. The set
   reported `avplay=0 filesystem=1` — the contradiction that gave it away — and playback
   would have used a `<video>` element that cannot open a local file on a TV.
3. **The set's identity was unreachable.** `webapis.productinfo` needs its own
   privilege (`http://developer.samsung.com/privilege/productinfo`); without it every
   read returned nothing and the boot record said `model=unknown`. The value it returns
   is now kept honestly: `product=3.7` is the **product** version and is no longer shown
   under a "Tizen version" label, which would have been a confident false receipt.

What this client now does off-device, and therefore does not have to be discovered on
the television: it walks a mounted drive, parses film and episode identities from
filenames, stores the catalogue and the scan report in one durable document, keeps
item ids across rescans so positions stay attached, marks a pulled drive's rows
unreadable without deleting anything, opens a sidecar subtitle file where the decoder
accepts one, and offers an audio/subtitle menu whose choices are remembered for the
next file.

Three things the gate still covers completely, because only the television can answer
them: whether the platform grants `filesystem.read` on a USB drive at all, whether
AVPlay accepts the URI the platform's `toURI` returns, and whether the engine renders
this bundle.

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
