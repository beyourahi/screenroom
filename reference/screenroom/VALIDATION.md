# Validation — v0.1 foundation

## Passed in this authoring environment

- Svelte/TypeScript diagnostics: 0 errors, 0 warnings.
- Six unit tests: release-name parsing, conservative episode ordering, source-aware rescan merging, progress normalization/completion, backup validation, profile isolation.
- Production shared-web build.
- Classic-script TV build and self-contained offline `preview.html` generation.
- Headless Chromium smoke checks using generated local WebM test patterns: empty state, real folder import, parsed episode labels, watchlist filtering, actual video decoding/playback, saved progress, reload/reconnect/resume, next-episode autoplay, profile isolation, search.
- Browser layout checks at 1440 px, 3440 px, and 360 px. No horizontal overflow at 360 px. Desktop and ultrawide screenshots visually inspected during development; screenshots are not in this deliverable.
- Mocked Tizen storage enumeration, storage scan, AVPlay dispatch, progress checkpoint, and closing playback. These checks validate adapter wiring only.
- No uncaught errors observed in the shared-browser smoke flow.

Approximate frontend sizes: shared-web JavaScript 74 KB raw / 28 KB gzip; TV JavaScript 72 KB raw / 28 KB gzip; CSS approximately 14 KB raw. These figures exclude Rust/native runtimes, operating-system webviews, media, and optional artwork. They do not establish playback performance or native installer size.

The normal Playwright browser download could not be extracted in this environment. The smoke checks instead used a locally extracted Chromium dependency through `SCREENROOM_CHROMIUM_EXECUTABLE`, using the same Playwright test script included here. The app itself does not depend on this fallback.

## Not verified / not implemented

- Rust compilation and Tauri native launch: Rust/compiler and native platform prerequisites were unavailable here. No Windows executable, macOS application bundle, signed installer, or Cargo.lock is delivered. Generate and review the native lockfile on the first successful native build.
- Physical Windows PC, its GPU, ultrawide monitor, Windows version, HDR/audio chain, and real collection. Those specifications were unavailable in the retrieved device records.
- Physical Samsung UA50AU8000RXER: installation, permissions, USB filesystems/hot-plug, remote behavior, AVPlay surface compositing, codec/audio/HDR support, and suspend/resume.
- Samsung's actual Chromium version was not run here. JavaScript targets Chrome 76; TV output avoids module loading, external fonts, CSS aspect-ratio dependence, and critical flex-gap dependence. This is preparation for testing, not certification.
- Physical MacBook Air M4 and any iPhone. Mobile packaging is not part of this milestone.
- Native broad-format decoding, FFmpeg conversion, offline artwork persistence, automatic metadata enrichment, online subtitle search, intro/credit skipping, recommendations, previews, casting, PiP, SQLite, large-library virtualization, robust moved-file relinking, and parental controls.
- Live TMDB account/token calls; no user credentials were supplied or stored. Cached artwork retention and distribution licensing remain unresolved. TMDB logo attribution is still pending.
- Dflix access, operator permissions, media rights, login/download/streaming integration. The source screen has a bookmark only.
- Scans currently stop/report an error on unreadable directory entries; they do not yet return a partial-success report. Choose the readable media folder rather than an entire protected system volume.

## Hardware acceptance sequence

1. On Windows, optionally run `scripts/device-info.ps1` to retrieve CPU/GPU/RAM/Windows version and reported display dimensions without serial numbers, IPs, or product keys. The actual monitor model still needs to be recorded.
2. Compile and open the Tauri shell; confirm folder permissions and safe file serving, then integrate/verify a native media engine for the real file formats. Do not treat browser WebM playback as broad-format proof.
3. Sign/package/install `tv-build` on the AU8000; test one local/USB file, seek, resume, audio/subtitles, remote navigation, power/suspend and source disconnection. AVPlay needs actual hardware.
4. Compare the same representative collection across Windows and Samsung. Keep original media unchanged; use optional compatible copies when conversion is eventually implemented.
5. Validate the optional Mac build. Add phone work only when it becomes useful.

This is a working shared-interface foundation with native adapter source, not a finished production media application.
