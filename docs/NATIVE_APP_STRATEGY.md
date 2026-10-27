> **SCOPE AND STACK NOTICE (2026-10-26):** [`ADR-002_RUST_FIRST_STACK.md`](ADR-002_RUST_FIRST_STACK.md) as amended by [`ADR-003_DESKTOP_AND_TV_SCOPE.md`](ADR-003_DESKTOP_AND_TV_SCOPE.md) is the authoritative architecture: desktop/laptop (Windows, macOS, Linux) and smart TVs (Samsung Tizen, LG webOS, Android TV, other compatible TVs) only, with equal priority; shared Rust core, iced + wgpu + libmpv desktop, LightningJS 3 + Blits + TypeScript TV with platform-native playback, SQLite where supported, FFmpeg/FFprobe, MIT first-party code. iOS/Android phone and tablet apps and the Flutter fallback are out of scope. This document's product requirements, entities and evidence rules still apply to desktop and TV; any mobile, Flutter-only or Svelte/Tauri statement inside it is history, not a requirement. No physical-device gate has passed -- see `IMPLEMENTATION_STATUS.md` for what is actually verified.

# Installed-app strategy — native capabilities, no browser product

**Decision (2026-10-25):** Every supported target gets an **installed, launchable, local application**. This is a confirmed user requirement; stack and hardware feasibility are still provisional and UNTESTED.

## Meaning of native

A native-*installed* experience must not demand Chrome/Edge/a browser tab, a PWA, a hosted website, an internet connection, a local HTTP server or an account for normal use. Media stays in explicitly selected folders/USB and is read in place; optional online metadata/subtitles are not application-runtime dependencies.

Technical distinction: installed apps may contain *embedded web-based UI*. That is not a remotely served website but is also **not** a pure native-widget renderer. The owner values performance and visual quality more than an arbitrary language purity test; treat both honestly.

| Target | Installed artifact | Interface | Video engine | Evidence |
|---|---|---|---|---|
| Windows ultrawide (primary) | Tauri executable/installer candidate | Locally bundled Dropout/Svelte UI in OS **WebView2** | Native **libmpv** candidate (libVLC alternate) | UNTESTED |
| Samsung UA50AU8000RXER (primary) | Signed Tizen **`.wgt`** TV app | Packaged Svelte/DS UI in Samsung **Chromium M76-era engine** | Samsung **AVPlay** native video decoder | UNTESTED |
| macOS (optional later) | Installed desktop bundle | Bundled UI in OS WebView | Native player candidate | UNTESTED |
| iPhone/others (optional future) | True installed platform app | Adapt appropriately | Native media APIs | UNDECIDED |

**Samsung TV only accepts `.wgt` web-app packages, not `.tpk` native app packages** (Samsung official FAQ). A pure-native-widget Samsung TV implementation is not an option through the officially supported TV application path. It can nevertheless be a true installed app and use AVPlay to decode video natively. Tauri desktop shells similarly render UI in an OS WebView while native Rust code handles privileged capabilities and media.

## Recommendation

**Keep** Svelte 5 + private Dropout DS UI in Tauri/Rust + native desktop media player for Windows, and a separate installed Tizen UI + AVPlay adapter for Samsung. Domain, behavior, design semantics and much of UI can be shared across both packaged apps inside a *single monorepo*. This aligns with the actual design system and avoids shipping a separate full browser engine in each desktop package. It is a recommendation **subject to measured evidence**, not a claim of measured superior performance.

A Qt/QML or other all-native desktop renderer can be reconsidered if real Windows integration/performance gates fail. It cannot directly reuse Svelte DS components and would require a full parity/maintenance tradeoff; it **does not** change Samsung's `.wgt` limitation.

## Required acceptance gates

1. **Offline packaging:** app opens from installed launcher with standalone browser closed and network disconnected. UI/CSS/JS/fonts are packaged. No required `localhost`, CDN, hosted backend, active development server or PWA service worker.
2. **Native media:** Windows native player actually plays representative formats, audio tracks/subtitles, overlays and fullscreen/ultrawide. Samsung signed package actually enumerates permitted USB, obtains valid `file.toURI()` and plays via AVPlay. Browser HTML `<video>` alone is NOT enough.
3. **Measured speed:** cold/warm time to interactive; library input/D-pad key-to-focus latency; first video frame and seeks; frame pacing; real CPU/GPU/RAM use, including WebView and native player processes; dropped video frames if measurable. Compare identical datasets/content/settings. Do not invent numeric performance guarantees before measuring real devices.
4. **No UI drift:** the same canonical Dropout DS semantics and layouts render correctly on each target, with a validated M76 CSS/JS compatibility build for Samsung (no mandatory modern OKLCH rendering). Respect borderless focus, WCAG contrast and couch-distance navigation.
5. **Durability:** offline app/device restart, source disconnect/reconnect, permission loss, low storage, TV uninstall/update loss and backups all have tested outcomes, not assumptions.
6. **Resource priority:** native video and input take priority over indexing, automatic metadata enrichment, costly backdrop effects or previews. No more than one preview decoder, and graceful low-end-TV fallbacks.

**Sources:** [Samsung TV `.wgt`-only FAQ](https://developer.samsung.com/smarttv/develop/faq/application-testing.html), [Samsung web engine table](https://developer.samsung.com/smarttv/develop/specifications/web-engine-specifications.html), [Tauri architecture](https://v2.tauri.app/concept/architecture/), [Tauri OS WebView versions](https://v2.tauri.app/reference/webview-versions/), [Samsung AVPlay](https://developer.samsung.com/smarttv/develop/guides/multimedia/media-playback/using-avplay.html).
