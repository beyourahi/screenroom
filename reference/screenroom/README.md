# Screenroom — working title, v0.1 foundation

Personal offline media library. No media is uploaded. This is an early working foundation, not a completed player or a release certified for your hardware.

## Device priorities

1. Windows PC with ultrawide monitor — primary. CPU/GPU/Windows version and monitor resolution were not found in available records.
2. Samsung UA50AU8000RXER — 2021 AU8000, 50-inch 4K. Recorded firmware T-NKLAUABC-2301.1 / 2301.1. Target baseline Tizen 6.0, inferred from the 2021 platform; not read directly from the TV.
3. MacBook Air 13-inch M4 (2025), Mac16,12 / A3240, 10-core CPU, 8-core GPU, 16 GB unified memory, 256 GB SSD — optional desktop target.
4. iPhone 15 base, 128 GB — optional future target. No iOS build or installation flow included in this milestone.

Future device support is an architectural objective. No universal compatibility or lag-free guarantee is claimed.

## Architecture

- Svelte 5 / TypeScript / Vite: shared interface and library behavior. No server-rendering runtime, paid API, hosted backend, remote font, or cloud account required.
- Tauri 2 / Rust: desktop shell and user-authorized read-only folder indexing. The Rust compiler is not available in the authoring environment; this shell has not been compiled or tested.
- Samsung Tizen package: shared interface plus read-only storage enumeration and AVPlay adapter. Must be signed, installed, and tested on the real TV.
- Browser decoder: working browser-playback baseline. Native libmpv/libVLC integration is still pending. Browser/system codec restrictions apply, particularly MKV, multichannel audio, HDR, and embedded subtitles.
- `src/lib/platform.ts`: device operations isolated from product behavior. Additional platform adapters can be added without replacing the library model or design system.

Developer familiarity was not used as the selection criterion. Small assets, platform reach, playback options, and replaceable integrations drive this choice. AI-managed implementation still needs reproducible builds, maintainable dependencies, and validation.

## Run the shared app

Requires Node 22.12+ or a recent Node 24 release.

```sh
npm ci
npm run dev
```

Open the localhost URL printed by Vite. Connect a media folder. No example movies or fake catalog entries are bundled. Files are referenced in place and never copied to a server.

On a browser restart, reselect the same media folder to reconnect its files; metadata and per-profile history remain. A browser does not retain portable filesystem access merely because it saved a filename. Files moved/renamed or different folder selections can create new entries; fingerprint-based relinking is pending.

## Offline visual/playback preview

Open `preview.html` in a modern desktop browser. It contains the actual compiled interface and works without a development server or internet connection. Browser codecs apply. To regenerate it, run `npm run preview:build`. Choose the same media folder again after a reload to reconnect files. This is a browser preview, not a native installer.

## Desktop shell

Install the normal Tauri prerequisites for your own OS, including Rust and the required platform build tools. Then:

```sh
npm ci
npm run tauri dev
# After on-device validation:
npm run tauri build
```

Build Windows installers on Windows and macOS bundles on macOS. No prebuilt binaries or signing identities are included. Folder selection grants asset access for the current app session; reselect the folder after restart. The source picker does not grant blanket filesystem access or follow symlinks. Codec handling is currently the system webview decoder; this is not yet a VLC-equivalent desktop engine.

## Samsung package

```sh
npm ci
npm run build:tv
```

`tv-build/` contains a classic-script IIFE bundle, local CSS, and a Tizen manifest. It does not require ESM loading from a `file:` origin. Use Tizen Studio / TV Extension and a Samsung development certificate tied to your TV to package, sign, and install it. The placeholder package/application identifiers must be adapted if the certificate tooling requires it. No certificate or signed `.wgt` is included.

Click Connect media, then select a storage and Scan selected storage in settings. TV storage APIs enumerate accessible locations only. USB filesystem, privileges, AVPlay, audio format support, suspend/resume, and remote keys still require actual AU8000 testing. If a mount disappears, history stays saved; reconnect and rescan. There is not yet a storage-change listener.

TV direction keys move focus between buttons, Enter activates, MediaPlayPause toggles, and Back/10009 closes playback. Native selection fields retain their normal controls. Ultrawide desktop playback keeps the original aspect ratio instead of cropping video to fill the display.

## Working in the shared app

- Read local folders, parse common movie and SxxExx / NxNN names, filter movies/series, search, sort.
- Title details, local profiles, watchlists, per-profile progress, Continue Watching.
- Resume, completion tracking, and conservative next-episode autoplay within the same season/source. Missing episodes are not skipped. Cross-season autoplay waits for validated episode ordering.
- Basic embedded HTML media controls and external UTF-8 SRT/VTT subtitle import. Image subtitles, complex ASS styling, language preferences, and native TV subtitle import remain pending.
- Periodic, pause, page-hide and visibility progress checkpoints. Browser playback pauses when hidden.
- Export/restore library metadata and history in the browser; original videos are never in the backup. Native export/restore integration is pending and is not shown in desktop/TV builds.
- Optional manual TMDB search and explicit match selection. The read token stays only in memory, never in backups. Queries go directly to TMDB.
- Dflix bookmark using the user-supplied URL. No account handling, content scraping, downloading, or availability claim.

## Desired scope, not implemented yet

Native broad-format desktop playback; FFmpeg conversion workflow; automatic metadata enrichment; durable offline poster/backdrop caching; episode-specific artwork; intro/credit markers and skip UI; OpenSubtitles search/download; local recommendations; muted browsing previews; PiP; casting; comprehensive source discovery and source-specific authorized download/streaming adapters; source hot-plug monitoring; robust file relinking; parental controls; mobile packaging; SQLite persistence and larger-library virtualization.

The foundation currently stores a versioned library snapshot in localStorage. Export backups before clearing app data. LocalStorage is bounded and not suitable for a large catalog; migrating to SQLite/IndexedDB is a planned storage-adapter change with the same data model. The current UI renders at most 120 search results; narrow searches for large collections. JSON imports are limited to 20 MB / 50,000 validated entries in this prototype.

No paid service may be enabled. Personal TMDB use requires attribution; full offline retention terms remain unverified. The current settings include the required notice, but the approved TMDB logo still needs to be obtained and added before any distribution. Artwork URLs are saved, not image bytes; therefore newly fetched artwork does not have a durable offline guarantee yet. Playback, basic organization, and history do not need TMDB.

Known parser limitations: ambiguous/remake titles, year-only titles, anime absolute numbering, multi-episode files, and specials need better parsing/manual correction. Show matching currently enriches the selected file only. Import is read-only and changes no original filenames.

## Verification

```sh
npm run check
npm test
npm run build
npm run build:tv
```

For the automated browser smoke checks: run `npx playwright install chromium`, `npm run preview:build`, then `npm run test:browser`. The included tiny WebM test patterns were generated with FFmpeg and are not movie content. Screenshots from the checks go to ignored `test-results/` and are not distributed.

See `VALIDATION.md` for actual results. A successful frontend build is not proof of native desktop, television, codec, battery, HDR, or ultrawide physical-device performance.

## References

- https://svelte.dev/docs/svelte
- https://v2.tauri.app/start/frontend/
- https://developer.samsung.com/smarttv/develop/guides/multimedia/media-playback/using-avplay.html
- https://developer.samsung.com/smarttv/develop/guides/data-handling/handling-usb-storages.html
- https://developer.samsung.com/smarttv/develop/specifications/media-specifications/2021-tv-video-specifications.html
- https://developer.themoviedb.org/docs/faq

Project licensing is intentionally undecided. No third-party movie, poster, Netflix/Dflix branding, or licensed media catalog is included.
