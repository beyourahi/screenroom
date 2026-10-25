# ADR-002 — Rust-first multi-platform stack (supersedes ADR-001)

- **Status:** Accepted
- **Date:** 2026-10-26
- **Supersedes:** [ADR-001](ADR-001_SELECTED_STACK.md) (Flutter-first), and every
  earlier Svelte/Tauri or Flutter-only statement.
- **Owner decision:** this ADR records the owner's finalized architecture. It is
  not a proposal. Historical documents that contradict it are retained only as
  history and are no longer requirements.
- **Product name:** Screenroom (finalized). Repository root:
  `~/Desktop/projects/screenroom`.

## Decision

Share the domain in Rust and give each platform its own idiomatic presentation
and its own native playback engine. No platform is a port of another platform's
UI, and no platform is deprioritized.

| Platform | UI | Playback | Storage |
| --- | --- | --- | --- |
| Windows / Linux / macOS desktop | Rust + [iced](https://iced.rs) (Elm-style) + wgpu | libmpv render API | SQLite via `screenroom-core` |
| iOS | SwiftUI | AVPlayer / AVPlayerLayer | `screenroom-core` through UniFFI; SQLite (bundled) |
| Android, Android TV, Google TV | Jetpack Compose | Media3 / ExoPlayer | `screenroom-core` through UniFFI; SQLite |
| Samsung Tizen TV | LightningJS 3 + Blits + TypeScript (`.wgt`) | platform-native player (AVPlay) | local storage through a documented adapter |
| LG webOS TV | LightningJS 3 + Blits (`.ipk`) | platform-native player (webOS media pipeline) | local storage adapter |
| Other TV platforms | platform-appropriate (native or web) chosen per device | platform-native | per device |
| Fallback | Flutter + Dart | per-device native | SQLite |

Shared logic: **Rust**. Schema: the single SQLite layout in
`crates/screenroom-core/src/schema.rs`. Media processing: FFmpeg/FFprobe as an
external free tool. Design: private Dropout Design System, mapped per platform —
never forked, never published. License: MIT for original first-party code.

Fallback rule: Flutter + Dart is permitted **only** when a platform cannot be
served by its designated stack with acceptable quality, and only with a recorded
reason. It is not a second baseline and must not become a parallel product.

## Why Rust-first

1. One typed domain, one SQLite schema, one set of invariants (identity,
   monotonic progress, non-destructive availability) for every client.
2. Desktop playback needs a real media engine and GPU compositing; Rust gives
   direct, auditable native access without a managed-runtime bridge.
3. iOS and Android get a single core through UniFFI instead of two
   reimplementations of the same rules in Swift and Kotlin.
4. TV browsers cannot host a Rust UI, so they get a web renderer that consumes
   the same HTTP-free, local-storage contract; the shared truth stays in Rust on
   devices that can run it, and in a documented SQLite-equivalent adapter on TV.
5. The previous Flutter candidate already proved the product shape; porting the
   domain to Rust keeps the validated behaviour while removing the cross-platform
   UI constraint.

## Verified facts at decision time (2026-10-26)

Verified by command on this host unless marked otherwise:

| Fact | Evidence |
| --- | --- |
| `rustc 1.99.1`, `cargo 1.99.1` available | `rustc --version`, `cargo --version` |
| `iced 0.14.0` is the current stable release (published 2025-12-07) | `cargo search iced`, [crates.io](https://crates.io/crates/iced/0.14.0) |
| `wgpu 30.0.1` current; iced 0.14 ships a wgpu renderer plus a `shader` widget | `cargo search wgpu`, [iced shader docs](https://docs.rs/iced/0.14.0/iced/widget/shader/index.html) |
| iced's shader widget exposes the real `wgpu::Device`/`Queue` in `Primitive::prepare`, so an externally produced frame can be uploaded as a texture | [Primitive trait](https://docs.rs/iced/0.14.0/iced/widget/shader/trait.Primitive.html) |
| libmpv render API supports **OpenGL**, **Direct3D 11** and **software** only — there is no wgpu/Metal/Vulkan backend | [mpv `render.h`](https://github.com/mpv-player/mpv/blob/master/include/mpv/render.h) |
| libmpv loads and decodes on this host, client API `2.5` | `cargo test -p screenroom-player --test libmpv_smoke` (see [IMPLEMENTATION_STATUS](IMPLEMENTATION_STATUS.md)) |
| `uniffi 0.32.2` current; UniFFI has full support for Kotlin, Swift and Python | `cargo search uniffi`, [UniFFI guide](https://mozilla.github.io/uniffi-rs/latest/) |
| `rusqlite 0.41.0` with the bundled SQLite feature (no system SQLite dependency) | `cargo search rusqlite`, workspace `Cargo.lock` |
| Lightning 3 renderer targets **WebGL 1.x+** and Chrome 38+/WPEWebKit 2017 as its floor, with Canvas as fallback | [lightning-js/renderer `BROWSERS.md`](https://github.com/lightning-js/renderer/blob/main/BROWSERS.md) |
| `@lightningjs/blits 2.14.0`, `@lightningjs/core 2.20.5` are the current published TV packages | `npm view` on this host |

Not verified, and therefore not claimed: Samsung Tizen 6.0 (2021) WebGL/AVPlay
behaviour with a Lightning 3 bundle, Tizen local-storage quotas, iOS/Android
toolchains on this host, Windows GPU/compositing, and any 4K/HDR path.

## Consequences

### Accepted

- Two UI technologies (iced, SwiftUI/Compose, Blits) plus one shared core. This
  is more code than one Flutter app, and it is the deliberate cost of native
  playback and native platform feel.
- The desktop video path needs an explicit bridge from libmpv to wgpu (see
  below). That bridge is now a first-class module, not an afterthought.
- TV clients do not run the Rust core; their behaviour must be kept in lockstep
  with a documented contract and contract tests.
- One schema for every platform. TV adapters must be able to read/write the same
  entities through their storage layer (SQLite where available, documented
  fallback where not).
- The Flutter candidate is archived, not deleted. Its domain rules and tests are
  the source for the Rust port.

### The libmpv → wgpu bridge (the main desktop risk)

libmpv can render into an OpenGL FBO, a D3D11 texture, or a CPU buffer. wgpu
draws through Metal/Vulkan/DX12. There is no zero-copy handshake between them in
libmpv itself, so the desktop app must choose per platform:

1. **Software bridge (portable baseline):** `MPV_RENDER_API_TYPE_SW` renders
   RGBA into a buffer; the frame is uploaded to a wgpu texture in iced's
   `Primitive::prepare`. Cost: one full-frame CPU copy per frame. Acceptable at
   1080p on modern CPUs; must be measured at 4K.
2. **OpenGL bridge (macOS first):** mpv renders into a GL FBO on a GL context
   created next to (not inside) the wgpu device; pixels are read back or imported
   via IOSurface. Required for hardware decoding on Metal-only hosts.
3. **D3D11 bridge (Windows):** mpv renders into a D3D11 texture that wgpu
   imports as a shared resource; avoids the per-frame copy.

No bridge is claimed to work until a real frame has been presented on real
hardware; see `FEASIBILITY_GATES.md`. Playback correctness (decoding, seeking,
audio, subtitles, duration) is proven separately and already runs headless.

### Design system

Dropout Design System semantics are mapped into iced styles, SwiftUI/Compose
presentation and a Blits presentation adapter. The private source stays out of
Git and out of public packages. A public build must not require it; the token
mapping lives behind an interface so the app still compiles without the private
pin. No screen's visual language may be silently replaced to satisfy a build.

## Alternatives considered

- **Flutter for every target (ADR-001).** Rejected by the owner. It forced one UI
  abstraction over TV and desktop and made native playback a plugin problem.
- **Svelte/Tauri everywhere.** Superseded twice: hosted-webview UI with a
  browser engine on desktop, and no path to Android/iOS.
- **Rust UI on TV via WASM + WebGL.** Rejected: bundle size, toolchain maturity
  on Tizen 6.0 and no real gain over Blits, which is built for these engines.
- **Per-platform full rewrite with no shared core.** Rejected: it re-implements
  identity, progress and availability rules three times.
- **Kotlin Multiplatform or .NET MAUI as the shared layer.** Rejected: neither
  gives the desktop media/GPU control the product needs, and both contradict the
  Rust-first decision.

## Exit criteria

This ADR stays "Accepted" while every claim above remains reproducible. It must
be revisited, with evidence, if any of the following is measured:

- iced/wgpu cannot present a libmpv frame above ~30 fps at the target resolution
  on real Windows hardware, and no bridge fixes it.
- Tizen 6.0 cannot run a Lightning 3 bundle at an acceptable frame rate, or AVPlay
  cannot play local media in that shell.
- UniFFI cannot expose the needed core API to Swift/Kotlin without unsafe
  hand-written glue on a supported OS version.
