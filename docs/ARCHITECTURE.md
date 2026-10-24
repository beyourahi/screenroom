> **2026-10-25 Screenroom decision supersedes outdated technology/name claims:** See [`ADR-001_SELECTED_STACK.md`](ADR-001_SELECTED_STACK.md) for the authoritative **Flutter + native platform media + SQLite** stack and `~/Desktop/projects/screenroom` path. Samsung 2021 officially supports Flutter `.tpk` apps; older “`.wgt` only” statements are WRONG and kept below only as historical proposal text. Dropout Design System remains the canonical visual spec, mapped to Flutter; its Svelte components cannot be used directly in Flutter. Windows/Tizen hardware performance and media features are NOT tested.

# Architecture and monorepo specification (provisional)

**Status:** implementation candidate pending `FEASIBILITY_GATES.md`. Platform integration claims are unverified. The design deliberately separates shared product behavior from native window/playback/TV APIs; it does **not** promise an identical binary or decoder on every device.

## Architecture goals

**Revised user contract:** a packaged *installed* application on each target device, no shipped website/PWA and no required network/localhost server. Native OS/video/file operations, shared private Dropout DS semantics and a compact Svelte UI where permitted. Do not confuse embedded system-WebView UI with fully native widgets; choosing to render UI natively would require a separate adapter and would reduce component-code reuse.

1. A **single Git repository/root** for every first-party program, package, Rust crate, source fixture, pipeline, test, spec and design artifact. All dependencies external via normal package registries/SDKs, no project submodules or sibling platform repos.
2. Offline-first by construction. UI is static/local and loads without internet. Media is read where located. Background network providers are replaceable; user-specific states remain local and independent across devices.
3. Player/persistence/source adapters respect OS capabilities. The domain does not import `window.tizen`, `window.webapis`, Tauri globals, arbitrary file URLs or direct `HTMLVideoElement` APIs.
4. Strong identity and durability for large, messy libraries: media path and TMDB ID are **not** primary identity. Different files/cuts/sources may map to the same Work but have different resume/markers.
5. Security and cost constraints: one root, no required hosted server, $0 external services, no implicit source file modification, bounded workload and clear controls.

## Proposed stack (not a lock)

**Important architecture boundary:** all rendering must remain *locally bundled* and all media indexing/decoding/storage local by default. Native video engine is independent of any WebView browser decoder. One monorepo packages distinct desktop and TV installed apps; there is no magic universal native binary. Keeping the Dropout/Svelte layer maximizes design parity. If native libmpv overlay + WebView fails a physical feasibility gate, evaluate an alternative desktop composition stack with a truthful DS parity/effort assessment; do not pretend the alternative embeds Svelte components without rewriting them. Details: `NATIVE_APP_STRATEGY.md` and `DROPOUT_DESIGN_INTEGRATION.md`.

- `Dropout Studio Design System` (private, immutable-commit vendored) + `Svelte 5 + TypeScript + Vite`: canonical Svelte UI components/tokens, screen state and accessible/remote navigation primitives. Document M76-compatible styling and composition adapters. TV output compiled/verified for **Chromium M76** (2021 Tizen); no SSR/cloud requirement. Current Svelte libraries/framework dependencies must be tested against that target; transpilation alone may not polyfill missing APIs or CSS.
- `Tauri 2 + Rust`: installed desktop shell with **embedded OS WebView for UI** (not a conventional website, but not native widgets either); secure commands, folder permissions, background indexing, native player integration and SQLite. Confirm actual WebView layering with libmpv; do **not** run broad-format video inside the browser decoder as the final goal.
- `libmpv` leading desktop native playback candidate; investigate child window vs render API and actual overlay geometry; `libVLC` as alternative. Native packages/builds/license obligations must be pinned and audited.
- `Tizen packaged Web application` (`.wgt`): **the supported installed-app form on Samsung TV (native `.tpk` is not supported on Samsung TVs)**; separate executable/client artifact from the same UI/domain packages; TV `Filesystem API` for authorized storage, `AVPlay` for native decoding, `wgt-private` for app-private data; no assumption of Tauri or arbitrary Rust binaries on TV. 1920×1080 AVPlay rect coordinate convention independent of video 4K panel resolution.
- `ffprobe/FFmpeg`: bounded stream probe and explicit conversion on a **computer**; never automatically transcode in the background during viewing.
- `SQLite` on desktop; **TV persistence subject to real gate**, likely file-backed versioned records + asset files, possibly IndexedDB after empirical testing. Samsung standard localStorage is only 5 MB. Follow provider retention rules for remote artwork.
- Mac can reuse desktop-adapter approach later after separate macOS build and native/media checks; mobile and other TVs need explicit adapters, not assumed compatibility.

## Monorepo layout (implement incrementally)

```text
screenroom/
  README.md  AGENTS.md  .gitignore
  package.json, lockfile, workspace config    # when runtime work starts
  Cargo.toml, Cargo.lock                      # when Rust work starts
  apps/
    desktop/             # Tauri + native player UI, actual Windows target
    tv-tizen/            # static TV client, manifest, signing instructions (no secrets)
    # NO browser/PWA product; optional test-only preview belongs under tests/preview/
    mobile/              # only after explicit future need
  packages/
    domain/              # pure TS domain model, matching, sort/search, progress logic
    contracts/           # versioned port/DTO interfaces and validation
    ui/                  # reusable Svelte screens, focus components, responsive system
    tizen-token-compat/  # compiled fallbacks DERIVED from upstream DS, not independent tokens
    # no browser/PWA client; internal preview adapter is test tooling only
    adapters-tizen/      # AVPlay/storage/remote wrappers
  crates/
    media-core/          # file probe/fingerprint/index/job logic
    desktop-adapter/     # SQLite/native playback/permission bridge
  tests/{unit,contracts,integration,e2e,fixtures}/
  tooling/               # local build/test/release scripts
  docs/                  # these plans + future ADRs/reports
  design/                # Codex-produced source design assets
  reference/             # original handoff + archived prototype, immutable by default
  .artifacts/            # ignored evidence/build reports
```

Use npm/Bun workspaces and Rust Cargo workspace only if appropriate; no heavy multi-repo task runner or paid CI. Root commands should expose lint/typecheck/unit/contracts/browser/native/TV/package checks with deterministic exit statuses. Run builds/tests on relevant OS/device; do not equate Linux or browser builds with Windows and TV installs.

## Dependency direction and ports

```text
 apps/desktop ---------+          +--- desktop folder/SQLite/player/FFmpeg adapters
                      |          |
 apps/tv-tizen --------+--> domain/contracts <-- TV filesystem/AVPlay/private-store adapters
                      |          |
 tests/preview --------+          +--- internal-only File API/HTML video test adapter
        packages/ui --> typed domain facade / capability service
```

**Core interfaces:**

- `SourceAccess`: `pickSource`, `listSources`, `enumerate(root,cursor)`, `relink`, `onAvailabilityChanged`. Typed scoped grants, never an unrestricted list of OS paths.
- `MediaInventory`: normalized file facts, probe jobs, stable *candidate* fingerprint, file/edition mapping. Scan may return **partial success** with per-entry errors and cancellation.
- `LibraryRepository`: transactions/migrations, cursor queries (not first-120), profile history, corrections, backup/restore and availability states.
- `ArtworkStore`: write/read verified **bytes**, provenance, provider ID, dimensions, retention/refresh limits, eviction, atomic metadata references. Local embedded/manual artwork can be a separate priority source.
- `MetadataProvider`: `findCandidates`, `getWork`, `getEpisode`, `fetchAsset` with quota/error/provenance; enrichment queue durable/restartable when online.
- `Player`: `open(mediaFileId,sessionId)`, `prepare`, `play`, `pause`, `seekMs`, `tracks`, `selectTrack`, `positionMs`, `stop`, events tagged with `sessionId`. Report capabilities and unsupported reasons per device/content.
- `SubtitleProvider`, `MarkerProvider`, `ConversionJob`, `CastSession`: opt-in and capability-gated; never silently put unsupported actions into menus.
- `PlatformCapabilities`: verified source types, decoder/profile/audio/subtitle/PiP/cast/secure store/backup capabilities. A platform label alone is not enough; probe actual device, content and permissions.

**Cross-language boundary:** typed, versioned DTOs (strictly milliseconds for public time) and validated IPC input; Rust owns privileged filesystem/process/SQLite access. Avoid returning thousands of full objects in each reactive render; expose paged/cursor queries and compact event updates. All async playback callbacks carry session IDs and an incrementing generation token so a late native event cannot change a newer media session.

## Persistence and lifecycle

`DATA_MODEL.md` specifies the entities. Source absence changes availability and last-seen state but **never deletes** Work/MediaFile/history. A manually corrected match has priority over any auto lookup. Folder rescans update file facts and candidate links, with a conservative reconciliation queue for collisions/renames. Do not hash every full file at launch. Editions distinguish cuts/runtimes; watch progress and markers must not transfer blindly across editions or transcodes.

Desktop uses SQLite transactions, schema migrations, integrity checks, backups. TV may use a versioned file-backed record store in `wgt-private` with checksums/atomic snapshot-or-journal writes, and artwork files separately; verify filesystem capabilities. Keep small settings in localStorage only where safe. Test application update/reinstall semantics and external recovery. No embedded heavy relational database should be assumed possible on Samsung Web runtime without evidence.

**Offline property:** compile/app UI/fonts/assets locally, store user progress/library and any **permitted actual image bytes** locally. Remote image URL alone is not offline retention. Network enrichment only from explicit configured sources; provider API error, rate limit or offline never blocks the main library.

## Source indexing strategy

1. User chooses root via native OS folder dialog / TV storage (developer-only browser preview may have File API), and grants scope. Preserve logical source identity independently of absolute path or Windows drive letter.
2. Nonblocking walk with bounded traversal/depth, path canonicalization and root checks, no default symlink following, duplicate root prevention, on-demand stream probe and robust partial-error collection.
3. Parse release names with year/show/season/episode hints and local NFO if present; classify **unknown** rather than misidentify. Build work and edition associations, with confidence + provenance and confirmation.
4. Enqueue optional metadata lookup for **entire connected collection**, dedupe by content identity, prioritize visible items and persist jobs. Retry/backoff quota/throttle. Fetch modest image sizes permitted by policy; job cancellation and stop on device resource pressure.
5. Record source disappearance/hot-plug as availability state; safely relink by heuristics and user confirmation. Do not delete originals, never auto-download torrents or other source content during scan.

## Playback and user-visible behavior

- Windows media decoder = native engine candidate; HW decode only if measured and supported by GPU + particular stream. Cover 10-bit HEVC, MKV, multiple audio/subtitles, Windows fullscreen/DPI and video-overlaid UI.
- Samsung decoder = AVPlay; handle `NONE→IDLE→READY→PLAYING/PAUSED` correctly, event ordering, absolute local URI, async prepare, correct video rect, remote/track selection, pause/resume and finish/cleanup; reject unsupported DTS-only streams with a helpful conversion option.
- Both use state machine `IDLE→RESOLVING→PREPARING→PLAYING↔PAUSED→ENDED/FAILED`, explicit session UUID. Progress checkpoint on appropriate lifecycle moments; no zero-time overwrite during prepare/seek; stale callbacks ignored; crashes may lose only bounded recent progress. Ended does not auto-play if no verified next consecutive episode.
- Remote/mouse navigation separated from content semantics; TV exposes focus graph and Back hierarchy; Windows ultrawide respects the movie's original aspect, optional crop choice.
- Browse previews are *optional advanced* and may be disabled on TV/weak machines. Exactly one short-lived decoder; still poster fallback. PiP/casting depend on real capability checks and user initiation.

## Network and security architecture

No mandatory server. A possible optional, temporary casting LAN endpoint is **advanced, explicit opt-in, session-scoped**, bound to authorized interface with token+timeouts/stop, never silently exposed to the internet. Metadata provider queries reveal title/ID/hash context; explain this. Remote source list must record consent/rights/protocol and distinguish HTTPS portal/FTP/torrent index/stream/CDN. Use external website handoff if no permitted API.

Sanitize imported metadata and subtitle content, validate paths/URLs against allowed origins, reject scheme changes/SSRF in future network work, limit file/entity counts during import, isolate web content from privileged Tauri context. Never pass filenames/URLs through shell interpolation; invoke FFmpeg with fixed executable and validated argument arrays. Secrets saved only where device capabilities securely support them and never added to backups/logs.

## Delivery and migration

Bundle native dependencies through a legally reviewed release process, not random binaries. TV signing certificates stay outside Git. App-generated data in OS-designated directories are not part of the project repo. `reference/` archives can be used for selective salvage after tests but not deployed. For every schema migration supply up/down or backup/rollback plan; TV dev reinstalls may erase private data, so document backup/export **before** reinstall.

## Alternatives and pivot criteria

- **libmpv → libVLC** if Windows output integration/codec/perf/redistribution problems persist. If both cannot compose with Tauri WebView, consider a native-window strategy or another desktop shell without changing the domain/UI contracts.
- **TV store** choose tested IndexedDB or private files rather than making an unsupported native SQLite promise; if free space is insufficient, limit cached image sizes and offer manual local poster support + backup.
- **Metadata** replace/disable third-party provider caching if terms prohibit the offline requirement, preserving manual and locally generated metadata/artwork paths. Do not evade terms by scraping.
- **Older/other devices** explicit capability subset rather than framework rewrite or deceitful universal promise.
