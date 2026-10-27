> **SCOPE AND STACK NOTICE (2026-10-26):** [`ADR-002_RUST_FIRST_STACK.md`](ADR-002_RUST_FIRST_STACK.md) as amended by [`ADR-003_DESKTOP_AND_TV_SCOPE.md`](ADR-003_DESKTOP_AND_TV_SCOPE.md) is the authoritative architecture: desktop/laptop (Windows, macOS, Linux) and smart TVs (Samsung Tizen, LG webOS, Android TV, other compatible TVs) only, with equal priority; shared Rust core, iced + wgpu + libmpv desktop, LightningJS 3 + Blits + TypeScript TV with platform-native playback, SQLite where supported, FFmpeg/FFprobe, MIT first-party code. iOS/Android phone and tablet apps and the Flutter fallback are out of scope. This document's product requirements, entities and evidence rules still apply to desktop and TV; any mobile, Flutter-only or Svelte/Tauri statement inside it is history, not a requirement. No physical-device gate has passed -- see `IMPLEMENTATION_STATUS.md` for what is actually verified.

# Design and interaction specification — Windows ultrawide + 10-foot TV

**Status:** Dropout Studio Design System is **confirmed and mandatory** for visual primitives, tokens, controls, and interaction philosophy (2026-10-25). Screen-specific compositions and final branding still require design/real-device validation; `Screenroom` is a backup codename only. Do not copy Netflix/Dflix trademarks, layouts pixel-for-pixel, licensed marketing/media assets or proprietary UI.

## Experience intent

An unmistakably cinematic, quiet and fast personal film library with a high-end editorial sensibility. Dense enough to browse content efficiently, never a SaaS admin dashboard or endless full-screen marketing hero. **Continue Watching at the top**, then user-selected library. Poster/backdrop artwork is a supplement to real locally playable content, not a precondition to show a file. When offline, the UI must look intentionally complete without any CDN/font request.

## Design source of truth — Dropout Studio Design System (confirmed)

**Canonical private upstream:** `https://github.com/beyourahi/dropout-design-system`, `DESIGN.md`, `GUIDELINES.md`, `src/lib/tokens/tokens.json`, source `src/lib/`, and `component-lifecycle.json`. Reviewed default-branch commit on 2026-10-25: `41406ab436ef2b9c1edcf69e9d4ada4b94d13eec` (provenance checkpoint, **not** permission to copy source into public planning materials). `docs/DROPOUT_DESIGN_INTEGRATION.md` contains the exact adoption and verification plan.

- Black-first, premium, editorial, restrained, typographic; content art supplies the cinematic richness. Canonical `.dark` theme, neutral ink/semantic surfaces and a deliberate `signal` action accent. **No pale-green prototype color scheme or invented local token ramp.** No copy/pixel replication of Netflix/Dflix.
- Consume semantic Dropout DTCG tokens and approved primitives/signature components/patterns through the private pinned vendored path (`src/lib/ds` in a supported consumer); no competing shadcn/variants, extra color utilities, local CSS kits or generic-component forks.
- Dropout-owned controls are **borderless, outline-less, ring-less** even in focus/error states. Separation via contextual semantic fills. Focus state uses immediate stronger fill and contrasting foreground. For rich focusable poster rails/details, follow DS `data-focus-container` marker model *when supported and verified*, otherwise use a reviewed equivalent platform adapter preserving the same high-visibility semantics without a forbidden border. **TV focus needs stronger couch-distance testing; it is not permission to silently violate DS law.**
- Conform to WCAG 2.2 AA contrast and semantic states; 44×44 coarse-pointer targets, D-pad navigation, visible selected/focused distinction, reduced-motion and forced-colors where applicable. Retain fluid heading hierarchy but choose TV typography/layout using physical-screen measurement, not desktop CSS scaling.
- Iconography: selected Dropout `MovingIcons` through DS barrelling only, otherwise static Lucide; no constant icon-animation on an old TV, no looping backdrop motion or excessive blur; budget one preview decoder.
- Svelte/Tailwind 4 and private DS styles require an **M76 compatibility path** on the 2021 Samsung TV (OKLCH, CSS variables, unsupported functions/selectors, JS APIs, Svelte dependencies). Compile compatible CSS tokens/fallbacks from canonical DS source **without manually modifying upstream tokens or introducing a different TV theme**; visually test on physical TV.
- All type, icon, poster/image sizes are adaptively budgeted; local assets/fonts only. Missing metadata must not harm readability. Motion/focus never slows down seeking, play/pause, Back or other instant operations.

## Primary information architecture and screens

1. **First launch:** one clear action to connect source; explain local storage/read-only permission and that metadata is optional, network optional, files remain on device.
2. **Home:** Continue Watching hero/rail if any, recent added, next episodes when available, then library. No giant marketing fold burying actual titles. For empty collections, one focused connection action + brief explanation.
3. **Library:** movies/series tabs, all records including unknown items, search with file-name fallback, watched/availability/genre filters, sort, scan progress and source health. 10k entries reachable; no hidden 120-card cap. Scroll/focus stable when metadata enriches.
4. **Title detail:** poster/title/year/overview, playable editions/source availability, tracks summary, resume/restart/mark watched, seasons/episodes if TV, alternatives if file offline or unknown. Unverified imagery clearly distinguishable from user-corrected identity.
5. **Season/episode:** deterministic ordering, specials and combined episodes, missing gaps clearly shown, action starts exact file and episode interval if known. Next episode never skips unavailable/unverified episode silently.
6. **Player:** intentionally minimal controls; large seek target, current/duration, pause/play, track/audio/subtitle controls, fullscreen/aspect, resume from prior timestamp, explicit skip marker when valid, next-episode countdown with cancel. Control overlay readable over bright/letterboxed video.
7. **Sources:** add/label/rescan/relink/forget, show last seen, permission/disconnect, partial failures and jobs. Forgetting a source never implies deleting its media.
8. **Fix match:** candidate metadata with confidence/source/year, choose manually, keep original filename, work/edition linkage with reversible correction, never hide uncertain title.
9. **Profiles/Watchlists:** local profile switch, separate content decisions, favorites, history, safe rewatch behavior.
10. **Settings/Jobs/Backup:** language, local asset/storage usage, privacy/network/provider, accessibility, subtitle preferences, conversion job view (computer only), durable export/import and error recovery. Source directory is advanced and a separate permission-conscious view.

## Navigation rules — Windows and ultrawide

- Logical focus order and DS-approved **borderless semantic focus fills** for keyboard; hover/pointer cannot be the only means of discovery. Distinguish focus, active playback, selection and checked state.
- Ultrawide: text/card grids constrained by comfortable reading width; use side panes, rails or additional posters sensibly. Video letterboxed/pillarboxed **at source aspect** by default, no stretched 21:9 fill; reversible crop mode only upon explicit choice.
- Esc/Back closes modal then details then returns to previous context; preserve last focused card and scroll offset across route, enrich update and settings return. Search keyboard shortcuts only if they do not hijack the player.
- After selecting or losing a file, show explicit outcome, never black empty screen or infinite spinner.

## Navigation rules — Samsung TV

- **Only a D-pad, Enter, Back and media keys are assumed.** No mouse or hover interaction required. All screens can be traversed deterministically from the initial focus item.
- Define a **semantic focus graph**: grouped rails/grid with explicit next focus target at edges, preserved last item per list, known trap/escape behavior in dialogs and on-screen keyboard. Geometric nearest-element fallback alone is inadequate.
- Always show clear selected focus state from sofa distance, prevent key-repeat skip from skipping selections, avoid focus loss during async metadata updates and virtual scrolling, maintain safe margins.
- Back precedence: close submenu/dialog → close details → exit player to prior focused item → previous screen → confirm app exit as appropriate. MediaPlayPause and seek operations use supported registered keys; test actual remote mapping.
- TV playback overlay must not assume web-based video stacking with AVPlay; prototype with real `object`/display rect. Use native subtitle controls only where supported and provide user-facing limitation/fallback for other formats. Pause/suspend must checkpoint progress.
- TV app UI convention may be 1920×1080 while underlying video is decoded/displayed at 4K. Treat them as different coordinate spaces.

## Critical states (each must be designed, not an afterthought)

| State | Required UI and behavior |
|---|---|
| Empty/no source | Explain how to connect local folder/USB; no fake movie rows. |
| Indexing/partial | Progress and cancel, usable partial results, partial-read errors, no main-thread hang. |
| Unknown or ambiguous | Keep filename/thumbnail and playable action; offer safe fix-match. |
| Not connected/mount missing | Retain artwork/progress, show offline badge and relink/rescan affordance. |
| Internet down/provider quota | Full local playback works; enrichment status indicates delayed/failed. |
| Unplayable audio/video | Actual stream-specific error; track alternative or explicit desktop conversion option. |
| Unsupported subtitles | Explain per-platform limitation; permitted import/convert fallback. |
| No/expired permitted art | Readable local placeholder, no broken remote URL glyph or infringing cached image. |
| Missing next episode | End playback/return to detail; never jump to wrong episode. |
| Corrupt import / no storage | Validation/error + non-destructive rollback / clear next action. |
| Reduced-motion/low-end TV | Same controls, disabled video preview/fancy motion, readable stills. |

## Accessibility, performance and QA

- Target ≥4.5:1 ordinary text contrast and ≥3:1 for sufficiently large text and essential UI boundaries, subject to actual screen evaluation. Keyboard/TV focus always visible; accessible labels for icon-only actions, readable subtitle overlays with backgrounds/sizing, Unicode/Bengali handling.
- Avoid persistent heavy blur, autoplay multiple decoders, huge uncached image bitmaps and full-page rerenders. Cap GPU layers and stagger asset loading, but never degrade control response during active playback.
- Interaction acceptance: route/Back/focus restoration, source loss, profile switching, hidden row virtualization, low metadata match, slow provider and offline restart on real hardware. Capture approved images/screenshots only using test fixtures and no personal watch history.

## Wireframes and tokens

Create editable design sources later in `design/` **within the same monorepo** after Codex CLI begins actual UI work. This package does **not** invent or approve final logos, illustrations, posters or wireframes; **colors, typography roles, spacing and motion must come from the canonical private Dropout DS tokens**, not new project-local choices. Use original/rights-cleared placeholder assets for tests only.
