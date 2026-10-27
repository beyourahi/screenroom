> **SCOPE AND STACK NOTICE (2026-10-26):** [`ADR-002_RUST_FIRST_STACK.md`](ADR-002_RUST_FIRST_STACK.md) as amended by [`ADR-003_DESKTOP_AND_TV_SCOPE.md`](ADR-003_DESKTOP_AND_TV_SCOPE.md) is the authoritative architecture: desktop/laptop (Windows, macOS, Linux) and smart TVs (Samsung Tizen, LG webOS, Android TV, other compatible TVs) only, with equal priority; shared Rust core, iced + wgpu + libmpv desktop, LightningJS 3 + Blits + TypeScript TV with platform-native playback, SQLite where supported, FFmpeg/FFprobe, MIT first-party code. iOS/Android phone and tablet apps and the Flutter fallback are out of scope. This document's product requirements, entities and evidence rules still apply to desktop and TV; any mobile, Flutter-only or Svelte/Tauri statement inside it is history, not a requirement. No physical-device gate has passed -- see `IMPLEMENTATION_STATUS.md` for what is actually verified.

# Archived v0.1 prototype — source review and salvage guide

**2026-10-25 correction:** prototype visuals/colors do not follow approved Dropout DS and must not become the design template. Browser-playback UI is noncompliant with the requirement for installed native-app UX and native media paths. Existing source may be referenced but cannot be blindly treated as a finalized product shell.

**Archive:** `../reference/prototype-v0.1.zip`; SHA-256 `e441d273f1d729ab61e7a1cb6924683d52e528c296a6d027afe892abdaf316a3`, 572,749 bytes, 47 entries under top-level `screenroom/`. This review inspected `README.md`, `VALIDATION.md`, `AGENTS.md`, package scripts, `src/lib/{model,metadata,platform}.ts`, native Rust folder-scan command and Tizen manifest. The original detailed inventory and historical validation context are in `../reference/SCREENROOM_CHATGPT_HANDOFF.md`.

## Existing behavior (historical, not re-run)

Shared Svelte/TS/Vite app, folder import via browser/native picker, basic filename/episode parsing, home and library/search/sort/filters, local profiles/watchlist/progress/resume, within-season conservative next-episode, HTML video and simple SRT-to-VTT/VTT subtitle import, browser snapshot export, manual TMDB match/search and remote poster URL, Dflix bookmark, Tauri Rust read-only folder scan, Tizen storage enumeration/AVPlay wrapper, development TV packaging, a standalone browser preview. Synthetic WebM fixtures. `VALIDATION.md` historically reports six unit tests and Chromium smoke checks plus mocked TV wiring; **not real device certification**. No Rust compilation or signed TV physical test recorded.

## Specific verified-by-source issues

| Source area | Problem | Replacement/refactor direction |
|---|---|---|
| `src/lib/model.ts` | `mediaId` = encoded JSON of `[source,path,size]`. Renamed/moved file, mount change or size change creates new identity; flattened `Media` lacks work/edition/episode mappings. | New UUID identity/domain schema with conservative relink and edition-aware progress (see `DATA_MODEL.md`). |
| `src/lib/model.ts` | `recordProgress` marks watched at >=95%; per-episode/cut/end-credits behavior not robust enough. | Session-scoped checkpoint and manual watched precedence, marker/post-credit handling. |
| `src/lib/model.ts` | Validator expects version 1 and up to 50k media entries, but storage is a whole-library snapshot. | Transactional desktop DB / TV private store and migrations; bounded import separate from live library size. |
| `src/lib/metadata.ts` | Manual TMDB selected-media matches and saved `image.tmdb.org` URLs; no durable actual image bytes, collection-wide worker or TV/season/episode detail model. | Provider abstraction, queued all-library enrichment, permitted binary asset store, attribution/rights research. |
| `src/lib/platform.ts` | Browser uses blob URLs; native scan uses `convertFileSrc` and webview decoding only, not VLC-level native codecs. | Implement true desktop player port and test layering on Windows. |
| `src/lib/platform.ts` | TV AVPlay code calls `setDisplayRect(0,0,1920,1080)`, assumes scan and URI work; no read-device evidence; `listFiles` failure aborts. | Signed physical Tizen spike, explicit URI and lifecycle tests, partial scan errors, retries, focus/resize. |
| `src-tauri/src/main.rs` | `WalkDir` failure returns whole scan error at unreadable entry; native scope session-only and no persisted durable root authorization/relink. | Partial-success scan, durable logical source with secure re-grant, nonblocking cursor. |
| `tizen/config.xml` | Development manifest and privileges, placeholder identifiers, no proof it installs or persists data on Samsung model. | Real dev cert/install/privilege audit, TV app-private filesystem storage. |
| `src/App.svelte`/UI | Prototype presentation and name unapproved; item cap historically 120; focus navigation not a complete TV spatial model. | Premium UX designed against M76/ultrawide, actual focus graph, virtualized complete search. |
| `VALIDATION.md` | Web/bundled JavaScript size and browser screenshots do **not** measure native installer, decoder/HDR/RAM/battery. | Exact on-device performance/codec receipts. |

## Reuse stance

**Recommended: preserve entire ZIP as immutable reference, then selectively salvage tested parsing ideas, Svelte UI fragments, test fixture patterns, read-only folder-picker permission concepts, Tizen API wrapper ideas—not the persistence/identity/player architecture wholesale.** No compiled `dist`, `tv-dist`, `tv-build`, `preview.html` or old `node_modules` should be made source-of-truth. Validate copyright/license/provenance for any archived asset before release.

- Preserve historical source bytes and link to this review rather than extracting ZIP on top of a new repo root.
- Do not treat `npm build` or TV emulator mocks as Windows-native/TV hardware proof.
- No implementation or prototype run performed in preparing these planning documents.
