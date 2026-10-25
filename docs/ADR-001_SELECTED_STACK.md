> **Status: SUPERSEDED on 2026-10-26 by [`ADR-002_RUST_FIRST_STACK.md`](ADR-002_RUST_FIRST_STACK.md).** Flutter + Dart is now a recorded fallback only. This ADR is kept as the historical record of the previous decision; do not implement it.

# ADR-001 — Screenroom's selected technology direction (2026-10-25)

**Status: SELECTED architecture baseline, NOT physically benchmarked or proven.** This ADR overrides prior provisional Svelte/Tauri/.wgt-only claims in the 2026-10-24 and v1.1 planning package. **Screenroom** is the owner's final chosen name, not a backup. Project lives in **`~/Desktop/projects/screenroom`** on the owner's Mac. The package retains exactly **one first-party Git repository** for all platform applications, platform plugins, tests, assets, docs, build scripts and native bindings.

## Decision and rationale

**Choose a native-compiled Flutter/Dart interface with platform-specific native video and filesystem adapters for the two primary devices (Windows ultrawide, Samsung UA50AU8000RXER Tizen 6.0).** This is the preferred launch architecture, not a guarantee of superior real-device performance. Desktop and TV use different hardware media engines even though they can share the Dart domain and Flutter UI. Build as installed applications with fully bundled UI/fonts/assets and zero required remote web backend or ordinary app-server. A web preview may exist only for tests.

| Target | Installed distribution/UI | Native video | Source + persistence | Priority |
|---|---|---|---|---|
| Windows ultrawide | Flutter desktop release binary, Dart AOT and GPU-rendered UI | libmpv via verified GPU-texture / direct renderer Flutter plugin or reviewed FFI implementation; prioritize native HW decoding, tracks, seek, overlays | Native folder picker and read-only enumeration, SQLite, asynchronous indexing, FFprobe + FFmpeg CLI for explicit conversion | FIRST |
| Samsung UA50AU8000RXER | **Flutter for Tizen** `.tpk` native-compiled installed application targeting **Tizen 6.0 (2021)** | Tizen-native `video_player_tizen` / Tizen Media Player API baseline, with tested version-appropriate native plugin for audio/subtitle/4K/USB; `video_player_avplay` alternative if justified and entitlement compatible | Tizen externalstorage permission & enumeration adapter; `sqflite_tizen` SQLite plugin or proven durable native app-private files; persist permitted local poster bytes | FIRST; HARD HARDWARE GATE |
| macOS | Flutter installed desktop `.app`, not a web tab | libmpv native GPU path candidate | SQLite, native file picker; FFmpeg optional | LATER OPTIONAL |
| Linux | Flutter desktop bundle | libmpv | SQLite, file picker | FUTURE |
| Android / Android TV / Google TV | Flutter native package with device-specific remote UX | Android Media3/ExoPlayer | SQLite, system access framework, platform files | FUTURE |
| iOS / tvOS | Flutter platform package if device support allows; SwiftUI if native platform requirements justify it | AVPlayer/AVKit | SQLite and sandbox/user granted files | FUTURE |
| Other TV OS (e.g., LG webOS, Roku) | Independently select OS-supported installed client stack; NEVER force Flutter where unsupported | Native receiver/player appropriate to OS | Local permissions and app-private state where possible | FUTURE; CAPABILITY GATE |

**Primary source:** Samsung's own Flutter for Tizen docs explicitly support **Tizen 6.0 (2021) and later**, `.tpk` installation and native-compiled UI. Samsung also documents Tizen .NET TV native applications. The older FAQ stating TVs only support `.wgt` contradicts these newer/more specific platform guides and must NOT be used to declare native TV apps impossible. A packaged **Tizen Web `.wgt` + AVPlay** is a measured *fallback* for THIS TV if Flutter fails, not the default. The exact device and app signing must be tested before choosing a distribution path.

## Why not retain Svelte/Tauri as the default?

- **Tauri 2 + Svelte + Rust + libmpv** is a valid, potentially low-footprint **desktop fallback** and directly reuses Dropout's Svelte component sources. However, Windows UI is rendered by an embedded WebView2; native-video layering, fullscreen, transparency and custom HUD composition need proof. Tauri 3 was pre-release alpha as of 2026-10-25, so a fallback would use stable v2.
- **Tizen `.wgt` + Svelte + AVPlay** is a mature TV fallback and directly reuses Svelte Dropout DS; on a 2021 TV it runs in **Chromium M76**. Modern Svelte/Tailwind/OKLCH DS syntax needs downleveling and real-device verification. It may prove leaner or more compatible than a Flutter engine on this exact TV, **so compare rather than make superiority claims**.
- **Qt Quick/QML + libmpv** is a legitimate high-performance Windows-native alternative if Flutter underperforms; it doesn't directly consume Svelte DS and open-source licensing/distribution obligations require care. A different UI toolkit does not automatically mean faster playback because native decoder/hardware pipelines dominate throughput.
- **Electron**, full bundled Chromium, and browser/PWA products add unjustified runtime surface or fail installed-native UX objectives. Avoid unless tests establish a compelling need.

## Dropout Studio Design System ownership — non-negotiable visual contract

The private upstream `beyourahi/dropout-design-system` is the canonical reference for design **semantics and visual rules**, including `DESIGN.md`, `GUIDELINES.md` and DTCG `src/lib/tokens/tokens.json`. The existing component implementation is **Svelte**, and **cannot be directly imported into Flutter**. Explicitly create and test a Flutter **presentation adapter** for its semantic tokens and documented component behaviors (black-first editorial typographic system, contextual neutral surfaces, borderless contrasting focus fills, reduced motion, scalable type/geometry). Product-specific cinematic UI (media rails, poster cards, player HUD, 10-foot TV remote navigation) belongs in Screenroom. Do not create a competing independent palette or falsely claim the Svelte DS components are being reused. When changes require upstream design governance, document and propose them through its private process. Keep DS source private; don't bundle a private source mirror in this public planning ZIP. A private immutable source SHA and reproducible mapping manifest should be recorded when implementation begins.

**Important nuance:** This favors the owner's visual specification and native renderer over literal Svelte component reuse. If the owner wants *the exact Svelte component source* rather than visual/component parity, choose the Svelte/Tauri/Tizen-web alternative and accept that rendering tradeoff. Neither option gets perfect native performance by definition; actual on-device measurements control release gates.

## Runtime and app architecture

- **Flutter/Dart**: UI, navigation, local domain model, metadata matching and orchestration, typed repository/playback/source APIs. Use **Dart isolates** for CPU-bound sorting/filename parsing/index preparation; virtualized rails/lists, decoded thumbnail size caps, cache budgets, and no constant expensive animated blur.
- **Platform native code only where valuable/required**: `libmpv` C API via reviewed plugin/FFI for Windows/macOS; Tizen native media / USB, audio track and subtitle APIs via maintained plugins or scoped C#/C++ bridge; Android Media3, Apple AVKit for future targets. Add **Rust** only if hardware profiling or safety-critical file/FFmpeg integration justifies it; do not force Rust simply to expand the language list.
- **SQLite** on each device (schema migrations, transactions, per-profile history, local artwork references, source identities and durable jobs). Validate `sqflite_tizen` on 2021 Tizen; native file storage fallback only if database plugin fails. Ensure backup/export and TV reinstall handling.
- **FFprobe + FFmpeg** bundled or verified local native executable for desktop media interrogation and user-triggered non-destructive conversions. Samsung TV performs no required transcoding; offer clear incompatibility guidance. Check exact mpv/FFmpeg binary flags and LGPL/GPL distribution obligations separately before any redistribution.
- **Optional networking** for metadata/subtitles/provider discovery only, never for basic launch, browse, local playback or remembered state. Local originals are NEVER silently uploaded, hosted, renamed or overwritten.
- **Free tools/services** for personal development and testing. No paid CI/backend/account/enrollment as required dependency. Package build tooling and Tizen Developer Mode setup need physical validation.

## Non-negotiable M0 feasibility tests before broad implementation

1. Obtain Windows OS/CPU/GPU/resolution/scaling/audio details (unknown), and launch a **release-mode** Flutter Windows app locally offline. Measure cold/warm start, idle RAM/CPU, 10k-item scroll, keyboard input latency, focus and overlay paint. Test libmpv GPU texture native video playback with realistic files, multitrack audio/subtitles, 4K/HEVC and fullscreen ultrawide controls. Check pause/seek/background resume, hardware decoding and power/thermal costs.
2. On the physical **Samsung UA50AU8000RXER** in Developer Mode, build/install/run a signed **Flutter Tizen 6.0 `.tpk`**. Verify USB enumeration, permission/mount lifecycle, local video decoding/4K, audio-track selection and external subtitles (the `video_player_tizen` public plugin docs currently say audio-track enumeration/selection APIs aren't supported), remote Back/D-pad and proper focus, app data/SQLite durability across power cycle, TV idle memory/launch/input smoothness. Test **real hardware** because Samsung's Tizen video_player plugins say TV emulator isn't supported.
3. Compare to a **minimal Svelte/Tizen `.wgt` + AVPlay** reference if Flutter is slow/incompatible or if essential track controls cannot be delivered within reasonable scope. Compare same files, same UI focus states, offline cold starts, measured memory, sustained playback and release artifact size. Keep whichever actually improves this specific TV experience, irrespective of framework reuse.
4. Compare Flutter Windows against **Tauri 2 + Svelte + native libmpv** only if a measured issue or player composition failure warrants it. Qt/QML remains a fallback only if both fail meaningful targets. Do not spend weeks benchmarking abstract frameworks; timebox the evidence experiments.
5. Define numeric acceptance budgets from initial observed hardware baseline, not fabricated 16-ms/60-fps/50-MB promises. Require smooth 10-foot input without lost keys, read-in-place/restore/disconnect resilience, accurate aspect ratio, robust resume and reasonable idle footprint; prioritize 1080p app UI with native 4K video path on TV.
6. Record supported and unsupported codecs/profiles/audio, disk privileges, subtitle formats and portability conditions. Never claim every format or device is supported until tested.

## Decision status

- **FINAL user choices:** name Screenroom, repository `~/Desktop/projects/screenroom`, ONE monorepo, installed local apps not browser-tab apps, Dropout DS visual foundation, offline local user-selected media, Windows/Samsung first, strictly free personal test budget.
- **SELECTED engineering baseline:** Flutter UI for both priority targets + per-device native decoder + SQLite + FFmpeg desktop; separate platform adapters; no mandatory server.
- **NOT YET VALIDATED:** exact media plugin behavior, package signing, GPU/remote performance on the actual 2021 Samsung, Windows hardware characteristics, compliant metadata caching/third-party licenses. Measured M0 findings may require a well-documented deviation, NOT silent noncompliance.

## Primary documentation (checked 2026-10-25)

- Samsung Flutter Tizen: https://developer.samsung.com/smarttv/develop/native/flutter.html
- Samsung Tizen .NET framework: https://developer.samsung.com/smarttv/develop/faq/tizen-net-tv.html
- Flutter Tizen toolchain: https://github.com/flutter-tizen/flutter-tizen
- Tizen video-player plugin: https://pub.dev/packages/video_player_tizen (track-control limitations; physical TV only)
- Optional TV AVPlay plugin: https://pub.dev/packages/video_player_avplay (version-dependent Tizen binary)
- Native Tizen SQLite: https://pub.dev/packages/sqflite_tizen
- Flutter Tizen plugin inventory: https://github.com/flutter-tizen/plugins
- Flutter official architecture: https://docs.flutter.dev/resources/faq
- mpv embedding methods and video rendering: https://github.com/mpv-player/mpv-examples/tree/master/libmpv
- mpv licensing: https://github.com/mpv-player/mpv/blob/master/Copyright
- FFmpeg licensing: https://ffmpeg.org/legal.html
- Tauri alternative (stable v2): https://v2.tauri.app/reference/webview-versions/
- Samsung M76 web runtime fallback: https://developer.samsung.com/smarttv/develop/specifications/web-engine-specifications.html
