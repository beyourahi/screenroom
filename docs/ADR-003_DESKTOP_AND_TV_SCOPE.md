# ADR-003 — Desktop and smart TV scope (amends ADR-002)

- **Status:** Accepted
- **Date:** 2026-10-26
- **Amends:** [ADR-002](ADR-002_RUST_FIRST_STACK.md). Its platform table, its
  UniFFI rationale and its Flutter fallback rule are superseded. Everything else in
  ADR-002 (Rust core, desktop stack, libmpv → wgpu bridge, TV stack, design system,
  exit criteria for desktop and Tizen) still applies.
- **Owner decision:** the owner finalized the platform scope. It is not a proposal.

## Decision

Screenroom ships for **desktop/laptop and smart TVs only**. iOS and Android
phone/tablet apps are permanently removed from the current scope. Desktop and TV
have **equal priority**.

| Component | Technology |
| --- | --- |
| Shared application core | Rust (`crates/screenroom-core`) |
| Desktop/laptop UI | Rust + iced + wgpu (`apps/desktop`) |
| Desktop playback | libmpv (`crates/screenroom-player`) |
| Smart TV UI | TypeScript + LightningJS 3 + Blits (`apps/tv`, not started) |
| TV rendering | WebGL, with Canvas fallback |
| TV playback | the platform's native player (Tizen AVPlay, the webOS media pipeline, Media3/ExoPlayer on Android TV) |
| Local database | SQLite where the platform supports it; a documented adapter with the same entities and invariants where it does not |
| Media processing | FFmpeg + FFprobe (external free tools) |
| Design system | Dropout Design System, mapped per platform, never forked or published |
| Build tools | Cargo + pnpm + the platform TV SDKs (Tizen CLI, webOS `ares-*` CLI, Android SDK/Gradle for Android TV packaging) |
| Architecture | offline-first, modular, one monorepo |
| License | MIT for original first-party code |

Supported platforms: Windows, macOS, Linux, Samsung Tizen, LG webOS, Android TV
(including Google TV) and other compatible smart TVs. Apple TV and Roku would each
need their own platform-specific implementation and are not in scope today.

## Consequences

- **No mobile clients.** `apps/ios`, `apps/android` and the planned
  `crates/screenroom-ffi` are dropped. None of them had been built, so no code is
  deleted.
- **No UniFFI.** Its only consumers were the phone apps. The desktop app uses the
  Rust core directly; TV clients do not run Rust and follow the documented core
  contract (unchanged from ADR-002).
- **Android TV joins the TV client.** It uses the same LightningJS 3 + Blits app as
  Tizen and webOS, packaged as an APK with a thin native host and Media3 for
  playback. The host's exact design (Android System WebView with a playback bridge,
  or similar) is open and is decided at the Android TV gate (G8), with evidence.
  This is why the Android SDK, ADB, Kotlin and Gradle stay in the toolchain.
- **No Flutter fallback.** The Flutter candidate stays archived on the `archive/*`
  branches and in `.worktrees/` as history and as the source of the ported domain
  rules. It is not a fallback any more. *(Amended 2026-10-29: the archive was deleted.)* The Flutter, flutter-tizen and .NET
  toolchains were removed from the development machine.
- **macOS stays.** Apple's Command Line Tools are enough for the Rust desktop
  build. Full Xcode and iOS simulators are not needed; they would only matter for a
  signed and notarized `.app` bundle at release time, which is a separate gate.
- **Gates.** G7 (iOS) is retired. G8 becomes the Android TV gate. Every other gate
  is unchanged.

## Exit criteria

Revisit only on an owner decision. A future phone or tablet client, an Apple TV
app or a Roku app would each need a new ADR. None of them may change the shared
core's contracts without one.
