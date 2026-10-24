# HORCRUX CONTEXT HANDOFF — Offline media application (Screenroom, working title)

HANDOFF v1 · Prepared 2026-10-24 · Research/planning continuity, not a development authorization

## 1. TASK

**ID:** SCREENROOM-CONTEXT-2026-10-24. **From:** the existing ChatGPT Work conversation. **To:** a new, normal ChatGPT conversation. **Status:** product discovery and research; a preliminary prototype exists, but the intended next deliverables are planning documents for later development. **Priority:** preserve the entire substantive project context and continue without restarting discovery.

Read this entire document and, if attached, inspect `screenroom-v0.1-foundation.zip`. Treat this handoff as the conversation baseline, subject to the user's newer corrections. Preserve the distinction between what the user requested, what an assistant proposed, what code exists, what was tested, and what remains unknown. A source-code artifact establishes its actual contents, not approval of its architecture or proof that it works on physical devices.

**Immediate mode:** discussion, research, brainstorming, requirements reconciliation, and eventual SDLC planning in normal ChatGPT. Do not start implementation, redesign the application, publish anything, or execute the prototype simply because this document mentions those activities. The user intends to use **Codex CLI for development and design later**, using the planning package produced through these discussions.

**Latest user request, preserved closely:** “I want only one repository for this, like a mono repo, in which the entire application will exist for all devices. So no matter what we do regarding this project, everything should stay within this one folder, one repository.” The user now wants to move all conversation and research to normal ChatGPT and asks for a very detailed handoff, with no important detail omitted. They also ask whether to upload the existing ZIP or paste its contents into the handoff.

**Artifact recommendation:** upload this Markdown document and the original ZIP as two attachments. Keep code in the ZIP; do not paste tens of thousands of lines or compiled bundles into conversational context. Read its documentation first and selectively inspect relevant source. The handoff records the artifact inventory, important implementation details, tests, and limitations. If attachments cannot be opened, state that limitation; never claim they were inspected.

**Handoff method:** comprehensive state transfer, not a transcript dump. Repetition and filler are consolidated; unique requirements, corrections, technical details, uncertainties, artifact names, and consequential history are preserved. This follows the user's saved handoff preferences. The HORCRUX marker is a remembered handoff convention; this application has not been renamed Horcrux and no unrelated Horcrux project is incorporated.

## 2. OBJECTIVE

### 2.1 The original product idea

Build a personal, primarily offline media application that feels like Netflix but plays a collection supplied by its user. Think **an advanced VLC-style media player with polished streaming-service organization and viewing conveniences**. It is a shell around accessible media, not a service that uploads or hosts the owner's movies on cloud servers.

The owner wants to test it personally first. Selling, public distribution, and even showing it to others remain undecided. Do not turn speculative commercialization into a prerequisite for the personal version.

The collection may contain ordinary downloaded files with poor names and incomplete embedded information. The user explicitly mentioned torrents, TorrentBD, and ISP “FTP servers,” and expects many files may be pirated. Preserve this actual context without assuming every file is infringing or that every reachable source permits integration. Local playback, source reachability, acquisition capabilities, and distribution rights are separate questions.

Users choose folders on the current device and/or connect USB sticks or external drives. The application discovers playable movies and television episodes in those authorized locations. The initial brain dump considered automatic whole-device scanning; later selections favor **letting the user choose where to look**. Whole-device surveillance is not the agreed default.

Offline, the app must open, browse the retained collection, show locally retained information/artwork, play accessible files, manage profiles/watchlists, and remember progress. When online, it should enrich **every item in the connected collection**, not only whatever has already been played. Fetch movie/show/season/episode identities, descriptions and appropriate imagery when available, then retain permitted information and actual image bytes for later offline use.

The user's wording “everything Netflix has” expresses ambition and experience expectations, not a precise promise of every proprietary Netflix feature. Translate it into explicit capabilities and acceptance criteria. Do not silently drop requested features, and do not promise features that depend on unavailable hardware APIs or licensed data.

### 2.2 User experience and technical priorities

- Highly aesthetic, cohesive, premium, cinematic interface.
- Very lightweight and fast, including browsing large collections and responding to a TV remote.
- Good video performance, hardware decoding where appropriate, accurate aspect ratio, usable subtitles/audio, reliable resume and autoplay.
- Initially useful on the owner's actual viewing hardware, especially Windows/ultrawide and Samsung TV.
- A durable architecture that can accommodate more operating systems and TV brands later without replacing the entire product.
- A completely AI-managed SDLC/codebase. The owner's familiarity with a programming language or framework is **not** a stack-selection criterion. Delivered performance and experience matter. Dependency quality, clear boundaries, testing and maintainability still matter because they affect those outcomes.
- Strictly free tools and services for personal testing. No required paid API, cloud service, subscription, AI inference, CI service, app-store enrollment or surprise billing.

### 2.3 Intended planning deliverables

The user originally asked for research only and explicitly said not to produce a PRD immediately. Later they clarified that the eventual goal **is a PRD and whatever starting SDLC files are useful**, which they can hand to Codex CLI. The latest move to normal ChatGPT places further conversation/research before development.

A reasonable planning package already discussed includes a PRD, architecture/monorepo plan, design/interaction specification, implementation milestones, test/acceptance plan, decision/research record, prototype review, and repository instructions. Editable design tokens and wireframes can be useful later. These are candidate deliverables, not all finished artifacts. Avoid producing documents just to create bureaucracy; make each useful to the coding agent and tied to an actual decision or verification need.

The user said they intend to use an “Astro model” in Codex CLI. That spelling appears in the conversation, but the exact intended model identity is not established here. **It does not mean the Astro frontend framework was chosen.** No model choice, reasoning setting or agent orchestration configuration is required by the product. Existing repository instructions also say not to prescribe a model or effort setting in generated development instructions. Keep the handoff model-independent.

## 3. DONE WHEN

### 3.1 Successful context transfer

The new chat can accurately state the product, current priorities, confirmed answers, constraints, prototype state, evidence gaps, source-access uncertainty, and next planning work without asking the user to repeat already answered questions. It reads this document fully, reconciles attached artifacts, identifies any inaccessible evidence, and continues from the current research/planning state.

It must not call the prototype production-ready, claim physical-device tests happened, present an assistant's proposed stack as a user-approved permanent decision, or restart the universal-device debate as though the latest priority correction never happened.

### 3.2 Eventual planning-package completion

When requested, produce a coherent set of documents with consistent scope, one monorepo, clear feature priorities, platform capability boundaries, free-service constraints, honest feasibility gates, traceable acceptance tests, and an executable next step for Codex CLI. Requirements must distinguish confirmed decisions from proposed defaults and open questions. The documents should enable building the whole application properly, with no hidden dependence on an always-on server or silently omitted advanced features.

### 3.3 Eventual application completion, not current status

A meaningful initial success is an actual evening of viewing: on the Windows ultrawide PC or Samsung TV, connect a collection, find a title, play with correct video/audio/subtitles, stop, reopen offline, and resume correctly. The app must handle an unplugged drive, a wrong metadata match, a missing episode, a crash and an unsupported file without losing state or modifying originals.

Primary-device acceptance requires real hardware evidence. A successful browser preview or mocked TV API is insufficient. A feature-complete claim requires each requested feature to work on its declared supported platforms, or an explicit limitation/fallback accepted by the owner. Optional Mac and phone support are reported separately.

## 4. CURRENT STATE

### 4.1 Conversation evolution and corrections

1. The user supplied a long brain dump about an offline Netflix-like shell, local files, USB, automatic metadata and artwork, resume, recently played content, and episode autoplay. They requested in-depth research, gap analysis and brainstorming, not immediate implementation or a PRD.
2. They initially emphasized “every device,” operating-system independence, beautiful visuals, high performance and old-device reach. They wanted to avoid a future total stack rewrite and asked whether everything could be Rust.
3. They added source discovery: TorrentBD and `https://cdn.dflix.live`, a suspicion that the latter may only work on their ISP, and a wish for a broad Bangladesh ISP/FTP directory and other download choices. They praised Dflix's Netflix-like UI/UX.
4. They asked what else needed discussion. A large question list felt burdensome; they asked for an interactive “forum or something” to answer. Their actual selections are consolidated below. Do not ask the same questionnaire again.
5. They later relaxed universal compatibility: start with their actual devices, Windows/ultrawide and Samsung TV most important; Mac optional/work-first; iPhone a nice-to-have they probably will not use.
6. They emphasized that AI handles coding, so stack decisions must optimize actual performance, aesthetics, reliability and reach rather than developer familiarity.
7. An assistant produced `screenroom-v0.1-foundation.zip`, an early source/preview foundation. The user responded, “Wait, what is this? What did you just give?” and clarified that they wanted planning documents to hand to Codex CLI. The prototype was therefore an over-early implementation artifact, not evidence of approval to treat it as the final application.
8. The user allowed either reusing or ignoring the ZIP, whichever helps build the application properly. They then made the single-folder/single-monorepo requirement explicit.
9. Work began drafting planning files, but the user now wants this comprehensive handoff to continue conversation/research in normal ChatGPT, with development and design reserved for Codex CLI.

There were short acknowledgments such as “Okay, let's proceed” and an apparent transcription “Camping.” They add no separate technical requirement and do not override these later clarifications.

### 4.2 Confirmed questionnaire answers

| Topic | User answer / consequence |
|---|---|
| Help obtaining new media | “All of them whichever is possible”; show all actually available options, otherwise the next best available option. Do not interpret this as a requirement to implement every acquisition protocol in the first release. |
| Collection location | Files stored separately on each device; USB sticks and external drives. |
| Progress sync | Keep devices independent initially. No required account or cloud sync. |
| Home | Continue Watching first, then the library. |
| Autoplay | Next episode; video previews while browsing. |
| Unplayable file | Offer conversion on the computer. |
| Extras | Skip intros/credits; subtitle search/download; watchlists/favorites; search/filter/sort; recommendations from the owned collection; PiP/casting; separate viewer profiles; recommend a sensible initial feature set. |
| Metadata language | English by default. |
| Budget | Strictly free tools and services. |

### 4.3 Actual devices and priorities

| Priority | Device | Known information | Unknown / caveat |
|---|---|---|---|
| Primary, frequent viewing | Windows PC with ultrawide monitor | This is where the owner most often watches. | CPU, GPU, RAM, Windows version, monitor model/resolution/scaling, audio and HDR chain have not been established. Do not invent them. |
| Primary | Samsung UA50AU8000RXER | 2021 AU8000, 50-inch 4K; recorded firmware `2301.1` / `T-NKLAUABC-2301.1`. | Live runtime, permissions, codecs and source behavior have not been inspected on this TV. Samsung's model-family documentation supports a Tizen 6.0 baseline; that is not a direct current-device measurement. |
| Optional desktop | MacBook Air 13-inch M4, 2025 | Recorded identifiers `Mac16,12` / `A3240`; 10-core CPU, 8-core GPU, 16 GB unified memory, 256 GB SSD. | Primarily a work machine under heavy AI workloads. Avoid aggressive background work. No physical app test performed. |
| Optional future | iPhone 15 base, 128 GB | Owner dislikes long-form viewing on its small screen and is concerned about battery drain. | Initial phone support is not a must-have and should not delay primary targets. |

These details come from prior retrieved device context and the existing artifact; they have not been freshly read from the devices during handoff generation. “Every TV” and old-device reach remain long-term architectural aspirations, not an initial certification matrix or a promise of zero lag on arbitrary hardware.

### 4.4 Completed, in progress and remaining

**Completed in the prior work:** discussion and explicit questionnaire answers; a provisional stack recommendation; a small source/preview prototype; browser-oriented checks documented in its `VALIDATION.md`; initial review identifying major gaps; research references and unresolved provider/device questions; partial SDLC document drafting.

**In progress at transfer:** consolidating requirements and preparing the transition to normal ChatGPT. The preliminary planning folder contains only five Markdown drafts; it is not a complete delivered planning package.

**Remaining:** continue research and make evidence-backed decisions; complete/reconcile the PRD and related useful plans when requested; choose the native-player integration and TV persistence approach through feasibility work later; record Windows hardware; verify actual Samsung installation/playback/storage; resolve provider retention and source-access questions; implement/test/package the actual application in Codex CLI.

**Not done:** no production app, Windows installer, Mac app bundle, signed TV package, physical-device certification, completed all-platform monorepo implementation, public repository, deployment, commercialization plan, or final approved brand/design.

## 5. CONSTRAINTS / DECISIONS

### 5.1 Hard requirements and scope boundaries

- **One folder, one Git repository, one product history.** Every first-party platform client, native wrapper, shared package, test, design source, document and build script belongs under that root. Do not split TV/mobile/desktop into separate repositories or use submodules as an escape hatch.
- External SDK installations, package caches, OS runtime app-data, certificates and user media can live where the operating system requires. This is not permission to create separate product repositories. Movies stay on selected disks; do not copy them into Git.
- No hosting or uploading the media collection. No mandatory cloud backend, login, paid subscription or cross-device sync for the initial app.
- Read originals in place. Do not delete, rename, move or overwrite them without separate explicit instruction. Conversion creates a separate copy.
- No required paid tools/services. Existing hardware, internet, storage and electricity are assumed; new hardware or paid enrollment is not silently authorized.
- Windows/ultrawide and the actual Samsung TV first; Mac optional; phone optional future. Preserve future portability without scaffolding every possible brand before shipping anything useful.
- English metadata by default. Local profiles are desired, but are not automatically a parental-control or security system.
- Do not equate visible polished UI with working capability. No dead action buttons, mocked success states, fabricated library entries or unsupported feature claims.
- Keep difficult requested features visible in the backlog/capability matrix. A staged release is a checkpoint, not permission to quietly abandon them.

### 5.2 Provisional stack recommendation and reasoning

**Proposed, not permanently approved:** Svelte 5 + TypeScript + Vite for a compact shared interface; Tauri 2 + Rust desktop services; a real native media engine behind an adapter; a packaged Samsung Tizen web client using AVPlay and Tizen APIs; local durable persistence; replaceable online providers.

The user asked whether the whole application could be Rust. Rust is well suited to desktop indexing, persistence, probing, background work and native services. That does not make one Rust executable a universal Samsung/other-TV/mobile runtime. The core portability strategy is shared contracts, domain behavior, assets and design plus platform adapters, all in one repository. This is not a promise that every platform uses identical rendering or decoding code.

Tauri is the desktop shell, **not a Samsung TV runtime**. Its webview is the interface layer; relying solely on HTML video would not satisfy the broad-format VLC-like goal. The proposed primary desktop playback candidate is embedded **libmpv**, with **libVLC** a bounded fallback if integration evidence favors it. Neither is integrated or proven here. Check rendering/compositing with custom controls, native-window layering, fullscreen, resizing, DPI, track support, hardware decode, binary provenance, packaging and licensing before committing a large UI to the approach.

Use ffprobe/FFmpeg for stream inspection and explicit conversion rather than reimplementing codecs in Rust. Exact third-party builds and their licenses matter. Browser/frontend bundle size alone does not prove installer size, memory use, decode speed or TV performance.

For desktop persistence, SQLite through Rust is the proposal. For TV, prove supported durable storage first: a suitable native SQLite service/plugin only if actually available, otherwise a tested IndexedDB/local-asset approach behind the same repository contract. Do not assume arbitrary native SQLite is accessible in a Tizen web package or that cached browser data survives power cycles reliably.

Other cross-platform approaches were part of the broad stack question, but this handoff does not possess a complete verbatim record of every earlier assistant framework comparison. Do not fabricate a Flutter/Qt/Electron benchmark or a user rejection that is not recorded. The concrete recommendation above is a candidate with feasibility gates, not a measured winner for all hardware or a permanent freeze on dependencies.

### 5.3 Proposed monorepo layout

This is an architectural proposal, not the structure of the existing ZIP:

```text
screenroom/
  README.md
  AGENTS.md
  package.json / package-lock.json
  Cargo.toml / Cargo.lock
  apps/
    desktop/
    tv-tizen/
    preview/
    mobile/                  # only later, when useful
  packages/
    ui/
    domain/
    contracts/
    design-tokens/
    adapters-browser/
    adapters-tizen/
  crates/
    media-core/
    desktop-adapter/
  tooling/
  tests/{unit,contracts,integration,e2e,fixtures}/
  docs/                      # PRD, architecture, ADRs, status, capability matrix
  design/
  reference/                 # optional historical prototype
  .artifacts/                # ignored build/test/device evidence
```

Root npm workspaces and a Cargo workspace with simple scripts are sufficient candidates; a heavy orchestration service is not required. Create actual platform projects when needed rather than empty speculative scaffolding. Keep certificates/secrets/private evidence ignored. Initialize and review Git state in the actual future project; no repository was created here.

### 5.4 Shared boundaries and data design

Keep the UI/domain independent of Tauri globals, DOM video elements, Samsung APIs and absolute file paths. Proposed typed ports cover source authorization/inventory, durable library repository, playback, local artwork, metadata, subtitles/markers, durable jobs, PiP/casting and credential storage. Expose explicit platform capabilities instead of inferring feature support solely from brand names.

Use explicit time units—milliseconds at contract boundaries—and request/session IDs so stale events cannot update a new playback session. Validate native IPC input. Use argument arrays for workers; never interpolate filenames/URLs into shell commands.

Proposed entities and relationships:

- `Work`: movie or series with stable application UUID; provider IDs are mappings, not primary identity.
- `Season` / `Episode`: explicit order, specials and alternate/absolute ordering support.
- `Edition`: cut/release/runtime identity needed for correct progress, markers and conversions.
- `Source`: stable logical drive/folder identity, authorization/locator, availability and last seen.
- `MediaFile`: physical file identity, source-relative location, stream facts, size/time, sampled fingerprint and edition association.
- `FileEpisodeMap`: multiple episode intervals in a single file when known.
- `ArtworkAsset`: actual local bytes/location, linked entity, provider, hash, language, dimensions and retention/refresh facts.
- `Profile`, `PlaybackSession`, `Progress`, `WatchlistEntry`, `MetadataOverride`, `SegmentMarker`, `BackgroundJob`, `DirectoryEntry`.

Do not key history by title, raw path, provider ID or path+size JSON. Renames, drive-letter changes, rescans and metadata corrections must preserve appropriate identity. Fingerprints are heuristic; collisions and alternate editions need conservative reconciliation and sometimes manual confirmation. Avoid full-file hashing as a mandatory startup operation.

Persistence needs versions, transactions/migrations, recoverable backup/import, bounded validation and no silent schema-reset data loss. Local profile state must remain distinct. Unknown videos remain playable rather than disappearing because metadata matching failed.

### 5.5 Feature requirements register

The following IDs came from the partial PRD draft. They are a useful traceability scheme, not a previously user-approved release contract.

| ID | Required behavior and consequential details |
|---|---|
| F01 Sources | Choose, label, rescan, disconnect, reconnect/relink and forget authorized folders/drives. Forgetting must not delete originals. |
| F02 Discovery | Incremental, nonblocking, cancelable/resumable scan; useful results before completion; unreadable-entry partial success; symlink-loop/duplicate-root handling; incomplete-file awareness. |
| F03 Library structure | Real movies/series/seasons/episodes/editions/files, including unknowns, specials, anime/alternate order and combined episodes. |
| F04 Identification | Filename + directory context + embedded/local NFO/IDs when present; confidence/provenance; manual correction; background refresh cannot overwrite confirmed overrides. |
| F05 Enrichment | Queue the whole connected collection online; prioritize visible/recent items; rate limits, retry/backoff, cancellation and durable restart; show/season/episode details, not selected-file enrichment only. |
| F06 Offline assets | Persist permitted actual image bytes and metadata with provenance/retention rules; verify after restart without network. A saved image URL is insufficient. |
| F07 Playback | Proven native desktop engine and actual Samsung AVPlay path; play/pause/seek/tracks/fullscreen/aspect; honest unsupported combinations. |
| F08 Profiles/history | Stable local profiles with separate position, watched state, favorites, preferences/recommendation inputs. Periodic and pause/seek/stop/suspend checkpoints; stale-session safety. |
| F09 Home/resume | Continue Watching before library; resume/restart/mark watched/unwatched/remove; preserve file/edition/profile context. |
| F10 Autoplay | Known next-episode ordering, cancelable countdown, gaps/unavailable sources/ambiguity/end stop safely; cross-season only with reliable order. |
| F11 Library discovery | Search title/filename, movies/series, genres where known, watched/available filters, useful sorts; virtualized/paged but all results reachable. |
| F12 Lists | Per-profile watchlists/favorites survive restart, metadata changes, source disconnect and rescan. |
| F13 Tracks | Embedded/adjacent/manual subtitles; language/audio choice; subtitle size/position/timing offset as supported; Bengali/Unicode; declare image/ASS limitations by device. |
| F14 Skip markers | Intro/credits using permitted provider or imported/manual markers, edition/runtime validation, correction, post-credit handling and offline retention permissions. No data means no fabricated skip. |
| F15 Subtitle downloads | Free eligible provider, release/hash/language matching, candidates/quotas, user-triggered download, offline reuse; embedded/manual fallback. |
| F16 Recommendations | Local and explainable from the user's own collection/history/genres/favorites/availability; no paid AI dependency; useful cold start. |
| F17 Browse previews | Desired; proposed muted, intent-delayed, single active decoder; permitted local generation/assets; resource/reduced-motion/offline fallback to stills. |
| F18 PiP | Actual platform support; preserve controls/session; clearly labeled mini-window alternative where appropriate, no fake universal PiP. |
| F19 Casting | Actual receiver discovery/session, reachable file and compatible format; temporary authorized LAN serving may be needed; stop on session end, no required hosted backend. |
| F20 Backup/recovery | Versioned library/history/profile/preferences/corrections export/import; optional permitted assets; validate before transactional application; not a movie backup. |
| F21 Settings/secrets | Local playback/language/accessibility/provider/storage settings; device-appropriate secure credentials; omit secrets from logs/backups. |
| F22 Unavailable states | Distinguish disconnected drive, lost permission, missing file, provider outage and unsupported codec; retain history/info, allow retry/relink/rescan. |
| F23 Source directory | Expandable known/operator/community-provenance listings/bookmarks, type/access requirements, last check and current-connection status. Known endpoint checks only. |
| F24 Acquisition choices | Separate open/search/download/stream capabilities per source; all feasible options, next best honest fallback; HTTPS portal is not necessarily FTP; torrent index is not direct media hosting. |
| F25 Conversion | Explicit local computer job, separate output, target profile, estimates/progress/cancel/retry, output validation and edition/runtime mapping; remux or audio-only conversion before full video transcode when sufficient. |

The draft proposed F01–F13 and F20–F22 as a **core personal milestone**, with F14–F19 and F23–F25 in the broader personal target wherever feasible/free. This prioritization is an assistant proposal responding to the request to recommend an initial set; the user has not explicitly ratified every boundary. Never represent the core milestone as the entire requested application.

### 5.6 Proposed behavior defaults, not settled user decisions

- Muted previews; roughly 700 ms stable focus/hover delay; one preview at a time.
- Five-second next-episode countdown with Play now and Cancel.
- Pause on app backgrounding, subject to platform behavior and intentional PiP exceptions.
- Original aspect ratio with letterboxing/pillarboxing; explicit reversible crop/fill option.
- Manual skip button before optional automatic skipping; uncertain marker data must not trigger automatic skips.
- Completion around 95% only as a configurable heuristic, not a substitute for credits/post-credit logic or explicit watched state.
- Local, explainable recommendation ranking; no remote AI requirement.
- Optional spoiler-sensitive descriptions; not a user-selected must-have.
- One heavyweight analysis/conversion worker and a bounded enrichment queue, with watching/workloads taking priority.

These are useful starting assumptions, not reasons to ask the user a second long questionnaire. Resolve routine choices sensibly, label proposals and ask only consequential unanswered questions when needed.

### 5.7 Design context and platform ergonomics

The owner wants highly aesthetic visuals and likes Netflix and Dflix. **Dflix's actual UI was not inspected in this work.** The name “Screenroom,” the prototype palette, typography and layouts are unapproved proposals. Do not treat them as brand requirements or copy third-party branding/media assets.

The partial design proposal: dark restrained cinematic surfaces, strong poster hierarchy, one pale accent, local fonts/assets, readable typography, visible focus and modest motion. Suggested colors were background `#0d1010`, surface `#171d19`, elevated `#222a23`, text `#eceee8`, secondary `#a7b0a7`, accent `#d2dfb0`. Suggested 4/8 spacing, 6–12 px corners, 14–16 px desktop body text, 22–26 px TV text at a provisional 1920×1080 app viewport, and 120–180 ms small transitions. All are adjustable and unvalidated on the real TV.

Do not hide Continue Watching/library below a giant marketing hero. Design populated, empty, unknown-title, missing-artwork, partial-scan, offline, disconnected-source, wrong-match and unsupported-file states. Desired screens include first launch, home, library, movie detail, season/episode detail, player, sources/directory, fix-match, profiles/settings and conversion jobs.

Windows ultrawide: preserve video aspect ratio; use extra width for useful content rather than stretched text; constrain long descriptions/dialogs; support mouse and keyboard without hover-only actions; preserve scroll/focus through metadata updates and navigation.

TV: genuine couch-distance design, deterministic D-pad focus, Enter/media keys, correct Back hierarchy and focus restoration, remote-operable seeking, native-picker focus recovery, safe margins and held-key handling. App UI resolution is not the same as panel/video resolution. Every important action must be reachable without a mouse. A geometric nearest-button algorithm is a prototype aid, not sufficient production navigation.

Accessibility includes contrast checks (proposed 4.5:1 normal text, 3:1 larger text), keyboard/remote focus, labels, reduced motion and legible subtitles on bright video. Resource-saving fallbacks must preserve usability. Avoid expensive continuous blur/animations or a decoder for every card.

### 5.8 Source discovery and Dflix uncertainty

The user uses **TorrentBD** and their ISP source **`https://cdn.dflix.live`**. Preserve that exact CDN hostname. The conversation does not establish an exact verified TorrentBD URL or authorize inventing one. The user wants a broad list of ISP/FTP sources in **Bangladesh**, plus other available choices, and likes Dflix's experience.

Prior research recorded that DOT Internet's official site links to root `dflix.live`, and another ISP's list also mentions the root domain. That is evidence of links/listings, not proof that the precise `cdn.dflix.live` endpoint is publicly accessible, DOT-only, available on every BDIX network, or licensed for automated use. The exact CDN could not be meaningfully inspected from the research environment. **The ISP-exclusive-access claim remains unverified.** Failure from one environment does not prove restriction; a homepage loading does not prove a media file can be reached.

A future directory should record operator/provenance, protocol/service type, authentication and access requirements, last verification, current-device/current-connection results and supported actions. Distinguish reachable, inaccessible, login required, unknown, stale and not checked. Test known endpoints from the actual user's connection; network changes/VPN/ISP routing can change results. Do not infer routes from ISP branding/geolocation alone or scan arbitrary networks/ports.

“FTP server” is colloquial here: sources may actually be HTTPS portals, direct-file CDNs, torrent indexes or genuine FTP. Open website, source search, direct download and direct stream are separate integration capabilities. An external bookmark/client handoff is an acceptable next-best fallback. **An embedded BitTorrent client is not automatically required** by the user's desire to see all options; it is a separate decision with credentials, seeding, resource, network and distribution implications.

No comprehensive Bangladesh source inventory, verified operator permissions, live TorrentBD integration, Dflix catalog search/download/streaming integration or verified cross-ISP access matrix exists yet. Do not manufacture a nationwide verified list. Eventual distribution requires a separate review of provider terms, source rights and store rules; no media hosting by itself does not resolve those issues.

### 5.9 Other consequential engineering gaps

**Playback/format reality:** compatibility is a container + video codec/profile/level/bit depth + audio + subtitle + hardware/OS question. An `.mkv` or `.mp4` extension is not enough. Test actual HEVC/10-bit/4K/audio/HDR combinations and subtitles. Do not promise HDR passthrough, lossless audio, every codec or every old device without evidence.

**Source lifecycle:** hot-plug, changing drive letters/mount paths, permission revocation, inaccessible files, symlinks, duplicate roots, interrupted downloads, renamed/moved files and multiple cuts need explicit states and safe relinking. Avoid destructive cleanup simply because a drive is absent.

**Progress:** startup zero-time events, seek races, stale callbacks, switching profiles/files, crashes, suspend/power loss, rewatches and conversion runtime changes can corrupt resume. Model a session lifecycle and save robustly, not just a UI timer. Missing episodes must not silently autoplay past gaps.

**Metadata:** ambiguous/remake/year-only names, anime, specials, multi-episode files, embedded/local metadata, provider outages, quotas, API keys and corrections need conservative matching and resumable collection-wide enrichment. Provider-specific IDs cannot dictate app identity. Download only sensible image variants rather than every full-resolution asset.

**Offline retention:** provider access being free does not establish permission for indefinite offline asset retention. Cache actual permitted bytes and track provenance/refresh/retention. A provider incompatible with the required offline behavior must be replaced, licensed or explicitly limited, not silently worked around.

**Skip markers:** runtime/edition mismatches, recaps, credits/post-credit scenes and missing data require confidence/manual fallback. Local intro detection is optional later analysis, not a free exact feature on every TV.

**Casting:** many receivers need a network-reachable URL and codec support; local USB media on one device is not magically visible to another. A temporary authorized LAN endpoint can be consistent with “no cloud hosting,” but must not become an unrequested permanent server or progress-sync system.

**Conversion:** remux/container changes may suffice; sometimes audio-only conversion is enough. Preserve originals; check free space, cancel safely, validate output, account for sleep/interruption, and do not mix incompatible timestamps/markers across editions. Initially independent devices do not imply automatic remote control of the Windows computer from the TV.

**Security/privacy:** constrain IPC/filesystem roots, validate imports/URLs/redirects, bound parsers and background work, keep external websites out of privileged views, redact credentials and never ship them in backups. Directory-supplied addresses and explicitly chosen private endpoints need different trust treatment. Metadata queries can disclose title/ID/hash information; “no video uploads” does not mean zero network data.

**Release/distribution:** actual SDK/signing/install requirements, native dependency licenses, update paths, migrations and backup recovery matter even in an AI workflow. Public distribution/commercial use/store submission are later decisions, not automatically authorized by personal testing.

## 6. ARTIFACTS / REFERENCES

### 6.1 Original prototype ZIP: attach separately

**Filename:** `screenroom-v0.1-foundation.zip`  
**Size:** 572,749 bytes  
**SHA-256:** `e441d273f1d729ab61e7a1cb6924683d52e528c296a6d027afe892abdaf316a3`  
**Contents:** 47 files under a top-level `screenroom/` folder.  
**Existing download reference:** [screenroom-v0.1-foundation.zip](https://chatgpt.com/library?file_id=libfile_c24e58dc5b648191acb97dde18e4ec8d&account_id=personal).

This archive is a source/preview foundation, **not** a Windows executable, Mac installer, signed TV app or finished media product. Source and build artifacts are both present. Its `preview.html` is a compiled offline browser preview with browser codec limitations; it is not a native build or the planning specification the user originally expected.

Recommended receiving-chat reading order: `README.md`, `VALIDATION.md`, `AGENTS.md`, `package.json`, `src/lib/model.ts`, `src/lib/platform.ts`, `src/lib/metadata.ts`, relevant `src/App.svelte`/styles, native shell and tests. The source is inspectable without treating compiled bundles as primary documentation. Reading source is permitted; executing it is not the immediate research-chat task.

### 6.2 What actually exists in the prototype

- Svelte/TypeScript/Vite interface, manual folder import, common movie/episode filename parsing, movies/series filters, search/sort, title details and basic profile/watchlist behavior.
- Actual browser playback and basic resume/completion/Continue Watching, conservative same-season/same-source next-episode autoplay. Missing episodes are not skipped; cross-season ordering remains pending.
- Basic external UTF-8 SRT-to-VTT/VTT subtitle import and HTML media controls. This does not prove embedded/image/ASS support across devices.
- Periodic and pause/page-hide/visibility progress saves; browser playback pauses when hidden.
- Versioned localStorage snapshot, browser JSON backup/restore; native export integration not done.
- Manual TMDB search and explicit match selection; token kept in memory and not backed up. A saved remote poster/backdrop URL is not durable offline artwork.
- Dflix bookmark using the supplied URL, without scraping, account handling, download or streaming integration.
- Tauri/Rust shell source with a read-only selected-folder scan and session-scoped asset permission. Not compiled here.
- Tizen storage enumeration and AVPlay adapter source plus classic-script TV packaging. Not installed/tested on the actual Samsung.
- Synthetic tiny WebM fixtures and unit/browser checks. No licensed movie catalog, poster collection or Netflix/Dflix branding is bundled.

### 6.3 Prototype limitations and reuse assessment

**Recommendation already given:** keep it as optional reference and selectively salvage useful pieces rather than adopting the entire implementation wholesale. The user explicitly permits either reuse or ignoring it. No salvage/refactor has yet been performed.

| Area | Useful reference | Why it needs replacement or substantial work |
|---|---|---|
| UI | Compact Svelte interface, populated media flows, styles and keyboard/browser test ideas | Visuals/name unapproved; actual TV ergonomics and native-video layering unproven. |
| Identity/data | Parsing/merge/progress logic provides examples | Flattened physical-file model; `mediaId` encodes source/path/size JSON; moved/renamed files lose identity; no proper work/season/episode/edition separation. |
| Storage | Versioned snapshot and validated browser backup seed | Whole-library localStorage writes, bounded capacity, no transactions/SQLite/migrations suitable for the full product. |
| Scale | Simple search/sort useful for tiny libraries | UI renders at most 120 results; no proper virtualization; repeated index lookup in sorting can scale poorly. |
| Metadata | Explicit user match selection | Manual selected-item enrichment, naïve parsing, no full-collection durable queue, no episode-specific model or offline binary artwork store. |
| Desktop | Permission boundary and folder-picker/scan source | Browser/system decoder only; libmpv/libVLC not integrated; Rust not compiled; no native lockfile/installer. |
| TV | AVPlay/storage adapter and classic-script package idea | Mocks only; hardcoded assumptions including video display geometry; real USB, permissions, focus, power and decode not proven. |
| Failure handling | Existing disconnected/history behaviors | Scan aborts on unreadable directory entries; lacks proper partial-success report/cancellation/hot-plug listener; some errors are swallowed. |
| Playback state | Basic save/resume/autoplay flow | Needs session IDs and race protection; approximate 95% completion is not authoritative credits handling; geometric focus is insufficient. |
| Tests | Six unit tests and real browser playback flow, synthetic fixtures | Not comprehensive hardware, format, performance, security or recovery verification. TV mocks validate wiring only. |

If reused later, extract into an ignored reference directory within the one monorepo, not over the new project root. Port selected source/tests deliberately. Do not treat `dist/`, `tv-build/`, `tv-dist/`, a dependency cache or generated `preview.html` as a new production source tree. Keep any generated fixtures small and legally reproducible. No license for the first-party project has been chosen.

The prototype README says a future storage-adapter change could retain the same model. That statement does not erase the later review: the full product needs an evolved domain/identity model and migration, not just a localStorage-to-SQLite substitution.

### 6.4 Exact prototype inventory

All paths below are relative to the archive's `screenroom/` directory. Hashed build filenames are historical artifacts, not stable public interfaces.

```text
.gitignore
AGENTS.md
README.md
VALIDATION.md
dist/assets/core-Dv9Wrr_v.js
dist/assets/index-CT9OVeTi.js
dist/assets/index-DTw5-oVt.css
dist/index.html
index.html
package-lock.json
package.json
preview.html
scripts/device-info.ps1
scripts/package-tv.mjs
scripts/preview.mjs
src/App.svelte
src/lib/metadata.ts
src/lib/model.ts
src/lib/platform.ts
src/main.ts
src/style.css
src-tauri/Cargo.toml
src-tauri/build.rs
src-tauri/capabilities/default.json
src-tauri/icons/icon.icns
src-tauri/icons/icon.ico
src-tauri/icons/icon.png
src-tauri/src/main.rs
src-tauri/tauri.conf.json
tests/browser.smoke.mjs
tests/fixtures/Test.Show.S01E01.webm
tests/fixtures/Test.Show.S01E02.webm
tests/model.test.ts
tizen/config.xml
tizen/icon.png
tsconfig.json
tv-build/assets/core-Dv9Wrr_v.js
tv-build/assets/index-BeWk3fd1.css
tv-build/assets/index-D6YFWDAP.js
tv-build/config.xml
tv-build/icon.png
tv-build/index.html
tv-build/screenroom.css
tv-build/screenroom.js
tv-dist/screenroom.css
tv-dist/screenroom.js
vite.config.ts
```

### 6.5 Exact existing package configuration and commands

These are **historical prototype instructions**, not instructions to run anything in the new research chat, nor a guarantee of current package/API compatibility.

Package `screenroom`, version `0.1.0`, private, ES modules. Declared dependencies: `@tauri-apps/api ^2.0.0`, `svelte ^5.57.2`. Dev dependencies: `@sveltejs/vite-plugin-svelte ^6.0.0`, `@tauri-apps/cli ^2.0.0`, `@types/node ^24.0.0`, `playwright ^1.64.1`, `svelte-check ^4.3.0`, `tsx ^4.20.0`, `typescript ^5.9.0`, `vite ^7.0.0`. Ranges are not the same thing as exact resolved versions; use the attached lockfile when inspecting a reproduction.

The README requires Node 22.12+ or a recent Node 24. Existing scripts:

```json
{
  "dev": "vite --host 127.0.0.1",
  "build": "vite build",
  "check": "svelte-check --tsconfig ./tsconfig.json",
  "test": "node --import tsx --test tests/*.test.ts",
  "tauri": "tauri",
  "build:tv": "vite build --mode tv --outDir tv-dist && node scripts/package-tv.mjs",
  "preview:build": "npm run build:tv && node scripts/preview.mjs",
  "test:browser": "node tests/browser.smoke.mjs"
}
```

Recorded commands: `npm ci`, `npm run dev`, `npm run check`, `npm test`, `npm run build`, `npm run build:tv`, `npm run preview:build`, `npm run test:browser`, `npm run tauri dev`, `npm run tauri build`. The browser checks also use `npx playwright install chromium` when available. The earlier environment needed a local Chromium fallback through `SCREENROOM_CHROMIUM_EXECUTABLE`; this is a test-environment workaround, not an application dependency.

`scripts/device-info.ps1` is intended to collect Windows CPU/GPU/RAM/OS and reported display dimensions without serial numbers, IP addresses or product keys. It does not establish the actual monitor model. On restart, the prototype requires folder reselection in browser/native preview paths; saved filenames do not themselves retain filesystem permission.

Prototype imports are limited to 20 MB / 50,000 validated JSON entries. Source scans do not follow symlinks. TV build output is a classic-script IIFE suitable for packaged loading rather than an ESM assumption; the earlier compatibility target was Chrome 76, with certain modern CSS dependencies avoided. These are preparatory choices, not physical-TV certification.

### 6.6 Partial planning drafts: present but not delivered as complete

The prior Work workspace contains `/workspace/scratch/c944d824f37c/screenroom-handoff/` with:

- `README.md`
- `AGENTS.md`
- `docs/PRD.md`
- `docs/ARCHITECTURE.md`
- `docs/DESIGN.md`

They are assistant-authored planning drafts reflecting many proposals consolidated here. They are **not user-approved specifications**. References within those drafts to `docs/IMPLEMENTATION_PLAN.md`, `docs/TEST_PLAN.md`, `docs/DECISIONS_AND_RESEARCH.md`, `reference/PROTOTYPE_REVIEW.md`, `reference/prototype-v0.1.zip`, `design/tokens.json` and `design/wireframes.svg` refer to intended files that were **not yet created** at this transfer point. Do not describe them as an existing complete package or claim their links work.

These five drafts are not included in the original prototype ZIP and are not separate attachments in this handoff. Their substantive decisions and technical contents have been consolidated here. The old workspace path is provenance, **not a path a new normal chat or the user's computer can access automatically**. If exact draft bytes become necessary, ask for that artifact rather than pretending to read the old workspace. No completed kickoff prompt exists beyond the provisional README execution brief and repository instructions; do not invent a missing earlier implementation prompt.

### 6.7 Research references and evidence limits

The following primary/reference URLs were used or preserved in the prior research record. They are continuation leads, not proof that every page remains unchanged or that every service/hardware operation was tested. Handoff generation did not repeat live research. Recheck time-sensitive terms/APIs when the new research session addresses them.

| Topic | URL(s) | What the prior record supports / remaining caution |
|---|---|---|
| Samsung model baseline | https://developer.samsung.com/smarttv/develop/specifications/tv-model-groups.html | 2021 AU8000 family maps to the Tizen 6.0-era baseline; not a measurement of current installed firmware/runtime. |
| TV native playback | https://developer.samsung.com/smarttv/develop/guides/multimedia/media-playback/using-avplay.html | AVPlay integration guidance exists; source scaffold is not proof of actual decoding/controls. |
| TV USB | https://developer.samsung.com/smarttv/develop/guides/data-handling/handling-usb-storages.html | Storage APIs/guidance; actual permissions, mounts/filesystems and device behavior need testing. |
| TV media limits | https://developer.samsung.com/smarttv/develop/specifications/media-specifications/2021-tv-video-specifications.html | Model-year codec/stream limits are relevant; a container extension is insufficient. |
| Svelte | https://svelte.dev/docs/svelte | Official framework reference, not a measured superiority claim. |
| Tauri | https://v2.tauri.app/start/ ; https://v2.tauri.app/start/frontend/ ; https://v2.tauri.app/reference/webview-versions/ | Static shared frontend/OS webview/native boundary; no universal TV runtime or broad-codec guarantee. |
| mpv | https://mpv.io/manual/stable/ ; https://github.com/mpv-player/mpv | Native player candidate; embedding/bindings/build configuration and actual license obligations must be checked. |
| FFmpeg | https://www.ffmpeg.org/legal.html | Build/configuration-dependent licensing; do not infer blanket redistribution permission from a tool name. |
| TMDB | https://developer.themoviedb.org/docs/faq | Prior research indicated free noncommercial use with attribution and separate commercial considerations. Full retention/image terms were not resolved; do not invent an indefinite cache permission or a six-month limit. No SLA promise. |
| OpenSubtitles | https://opensubtitles.tawk.help/article/about-the-api | Prior docs showed limited free anonymous/account download allowances, with conflicting quota information elsewhere. Recorded figures included 5 anonymous / 20 free-account downloads daily; treat these as historical, unconfirmed account-specific numbers, not a product promise. Prefer the current .com REST API and verify live eligibility/terms. |
| IntroDB terms | https://introdb.app/docs/terms ; https://introdb.app/docs/api | Prior research reported player integration permitted, rate-limited lookup, and bulk database harvesting prohibited; verify current retention and use terms. It does not establish coverage or edition accuracy. |
| Apple optional testing | https://developer.apple.com/help/account/basics/about-your-developer-account/ | Free personal provisioning has limits; prior record noted seven-day provisioning/reinstallation constraints. Do not force paid enrollment into the $0 initial plan. |
| DOT Internet | https://www.dotinternetbd.com | Prior official-site link to root Dflix; not proof of CDN access from another ISP. |
| Another ISP listing | https://bdsmartlink.com/bdix/ | Root-domain listing only; not proof that all routes/media endpoints work. |
| BDIX peering | https://bdix.net/public-peering/ | Peering arrangements do not establish universal application/CDN access. |
| Exact user source | https://cdn.dflix.live | Exact access claim unresolved; root domain and this CDN are distinct evidence targets. |
| Possible future store policies | https://developer.apple.com/app-store/review/guidelines/ ; https://support.google.com/googleplay/android-developer/answer/9888072 | Relevant if distribution is pursued; no store submission, legal opinion or policy clearance has occurred. |

Metadata/marker/subtitle providers are candidates, not permanently chosen authorities. Attribution/logo requirements, quotas, credentials, retention, caching and future commercial use require current primary-source verification. The prototype's TMDB notice exists, but an approved logo asset and complete attribution/distribution readiness were still pending.

## 7. VERIFICATION

### 7.1 Historical prototype results, as documented

These are **recorded prior-environment results** from the prototype's `VALIDATION.md`, not tests rerun during this handoff, and not independent physical-device verification:

- Svelte/TypeScript diagnostics: zero errors and warnings.
- Six unit tests: release-name parsing, conservative episode ordering, source-aware merge/rescan, progress normalization/completion, backup validation and profile isolation.
- Shared web production build; classic-script TV build; self-contained `preview.html` generation.
- Headless Chromium flow using generated local WebM fixtures: empty state, actual folder import, parsed labels, watchlist filter, actual decoding/playback, progress, reload/reconnect/resume, next-episode autoplay, profile isolation and search.
- Layout checks at 1440, 3440 and 360 px; no horizontal overflow at 360 px; desktop/ultrawide screenshots inspected in that work.
- Mock Tizen enumeration, scan, AVPlay dispatch, checkpoint and close wiring. Mocks do not test actual firmware, decoder or storage.
- No uncaught errors reported in the shared-browser smoke flow.

Approximate historical size report: shared web JavaScript 74 KB raw / 28 KB gzip; TV JavaScript about 72 KB raw / 28 KB gzip; CSS about 14 KB raw. These exclude Rust/native dependencies, OS webviews, media and artwork. They do not establish total RAM, startup latency, native installer size, video smoothness or battery drain.

The normal Playwright browser download could not be extracted in the earlier environment; an available local Chromium dependency was used with the included test script. This failure/workaround is preserved so a new agent does not mistake it for an application limitation or assume a clean standard setup was verified.

### 7.2 Explicitly unverified or absent

- Rust compilation, Tauri launch, native-player embedding, Windows executable/installer, Mac bundle, signing, native Cargo lockfile.
- Physical Windows hardware, ultrawide native resolution/scaling, GPU/HDR/audio path and representative collection playback.
- Samsung installation/signing, privileges, USB filesystems/hot-plug, native AVPlay surface composition, tracks/subtitles, codecs/HDR/audio, remote focus and suspend/power-loss recovery.
- Actual Samsung browser engine behavior; targeting Chrome 76 in a build does not certify the TV.
- Any physical Mac/iPhone run or mobile package.
- Native broad-format decoding, conversion jobs, SQLite, durable offline artwork, automatic all-library metadata, robust relinking, large-library virtualization.
- Online subtitle downloads, real skip markers, recommendations, browse previews, PiP and casting.
- Live TMDB credential calls, full retention/licensing clearance, complete attribution/logo readiness.
- Dflix/TorrentBD access/integration, operator permissions, media rights or cross-ISP reachability.
- A comprehensive performance, security, recovery or accessibility audit.

### 7.3 Handoff-specific checks

The source archive's filename, file count, byte size, SHA-256 and inventory were checked while preparing this document. The local planning/prototype directories were checked for Git state; none was a Git repository. Package scripts and prototype documentation were read. These checks establish transfer provenance, not application readiness.

The final Markdown handoff is a newly created transfer artifact. The prototype was not modified, implemented further, built or executed as part of this context-transfer request. The exact final file's structural/content checks are reported separately when delivered; do not infer that later application tests happened.

### 7.4 Proposed verification work for the eventual plan

The following is **planning guidance consolidated from the discussion**, not evidence of completed tests or an approved benchmark contract:

- Native playback feasibility before broad implementation: actual Windows render surface, controls, fullscreen, resizing/DPI, tracks and hardware decode; actual TV USB-to-AVPlay path, persistence, focus and lifecycle.
- Legally generated/permitted representative media: H.264/AAC MP4, HEVC/10-bit/4K, MKV with multiple audio/subtitle tracks, conditional AV1, varied frame rates, partial/corrupt files, long files, SRT/VTT/Bengali Unicode and relevant ASS/image tracks. Test actual combinations and record unsupported ones.
- Offline relaunch with network disabled, retained binary artwork, no login/CDN/font dependency, and queue retry/backoff after connectivity returns.
- Moved/renamed files, changed mount points, duplicate roots, unplugged drives, denied permissions, unreadable entries, interrupted scans, wrong-match correction and multiple cuts.
- Progress/session races: failed open, startup zero event, seek/stop, profile changes, old callbacks, crash, suspend, post-credit scenes, explicit watched/rewatch, missing episodes and cross-season transitions.
- Large metadata libraries with every result reachable; proposed 10,000-item fixture and optional larger stress testing, not a 120-result cap.
- Practical resource budgets for actual hardware: bounded workers, one preview, hardware decode where available, no heavy analysis during playback/work pressure. Measure cold/warm startup, input latency, first frame, scrolling and sustained playback rather than assuming a tiny JS bundle proves speed.
- Provider and source failures: rate limits, missing matches, wrong release subtitles/markers, stale directory entries, homepage-versus-media access, current connection versus other ISP/VPN, and unknown versus unavailable.
- Import/migration rollback, corrupt backup handling, credential exclusion, validated IPC/paths/URLs, original-file preservation and cancellation cleanup.
- Deterministic TV navigation and repeated playback/suspend/reconnect cycles; ultrawide original-aspect verification; supported casts/PiP sessions on actual receivers/platforms.

No numerical startup/frame-rate/total-memory guarantee was measured or approved for the owner's hardware. Choose and label measurable targets when completing the test plan; do not retroactively present proposed targets as observed results.

## 8. FAILED / REJECTED / SUPERSEDED PATHS

1. **Premature implementation as the main deliverable:** the prototype ZIP arrived before the requested planning workflow was clear. The user questioned it and explained they wanted a PRD/plans for Codex CLI. Preserve useful code as optional evidence; do not continue building in normal ChatGPT by default.
2. **A giant question dump:** the user found many questions burdensome and requested an interactive answering format. Already answered choices are recorded here. Ask only missing consequential questions, preferably a small number at a time; do useful independent research without stalling.
3. **Literal every-device / zero-lag / forever-frozen stack:** superseded as a strict initial requirement by actual-device priorities. Long-term reach remains desirable; preserve replaceable platform boundaries rather than making impossible guarantees.
4. **Phone as an initial required target:** superseded. The iPhone is optional; Windows and TV drive the first useful release.
5. **Developer familiarity as stack justification:** explicitly rejected by the user. AI writes/manages code. Compare real product outcomes, while retaining engineering quality checks.
6. **Browser decoding as full VLC-like support:** insufficient. Working browser WebM is a baseline, not broad native format proof. Native engine integration remains a feasibility gate.
7. **Remote image URLs as durable offline artwork:** insufficient. Retained actual permitted bytes are required; retention rights remain unresolved.
8. **Path/size-based identity, localStorage snapshots, first-120 results as final architecture:** prototype shortcuts identified by review, not accepted product constraints.
9. **Treating a failed CDN inspection or root-domain ISP listing as verification of exclusive/universal access:** invalid. Exact access restrictions remain unknown.
10. **Separate repositories per device:** incompatible with the latest explicit monorepo requirement.
11. **Mandatory paid backend/sync/API/app-store membership:** incompatible with initial budget/independent-device requirements unless the user explicitly changes them.
12. **Blindly adopting all prototype branding/design/code:** not approved. “Screenroom” is a working title; selective reuse is permitted, wholesale commitment is not required.

## 9. NEXT ACTION

### 9.1 Immediate continuation in normal ChatGPT

Read this file fully. Inspect the attached ZIP's documentation and relevant source without executing it. Reconcile what is present with the handoff; identify genuine discrepancies rather than silently overwriting context. Briefly acknowledge the current mode and the few consequential unresolved questions. Continue research/brainstorming with the confirmed answers already in hand. Do not produce another prototype or start coding/design implementation unless the user explicitly asks.

The latest user has not asked the new chat to re-answer the entire research brief in one massive response. They want a crystal-clear continuation point and eventually high-quality documents to hand to Codex CLI. The next substantive work is to close important planning gaps and determine when the PRD/architecture/design/test/implementation package is ready to finalize, while avoiding unnecessary questionnaires or another unreviewed implementation detour.

### 9.2 Ordered future work, when authorized

This ordering is a proposal preserved for continuity, not a completed execution plan:

1. Reconcile scope, actual-device priorities, prototype reuse and the one-repository boundary; keep all desired capabilities traceable.
2. Establish a bounded native-player/TV-storage feasibility strategy and explicit evidence gates. Resolve provider retention/cost suitability and source-directory claims through appropriate research.
3. Finalize a coherent PRD and necessary architecture/design/implementation/test/decision documents. Use proposed defaults for routine choices, flag consequential uncertainty, and avoid promising hardware results before testing.
4. Later in Codex CLI, inspect the actual local repository/environment, preserve existing work, record Windows specs, and perform Windows-native playback and Samsung USB/AVPlay/persistence spikes. If hardware is inaccessible, continue independent work and report exactly which acceptance criteria are blocked.
5. Implement the monorepo foundations/contracts/persistence, local discovery/native playback, identity/enrichment/offline assets, profiles/history/library interactions, then advanced features/integrations/conversion and release hardening with meaningful checks.
6. Track implemented versus verified versus blocked/deferred capabilities, including actual codecs/devices, and package only what has evidence. Optional Mac and phone work must not block the primary personal targets without a concrete dependency.

Do not treat these bullets as a fully delivered milestone plan or assume every phase must be strictly serial. Do not silently drop advanced scope after a core checkpoint. Avoid prescribing a specific coding model or reasoning setting.

## 10. RISKS / UNKNOWNS / PROVENANCE LIMITS

### 10.1 Consequential unresolved items

| Item | Current state | How it affects continuation |
|---|---|---|
| Windows specifications | Unknown | Final native/performance/ultrawide certification needs actual device facts, not guesses. |
| Samsung live behavior | Unknown | Installation, source permissions, codecs, rendering, storage and lifecycle are hard gates. |
| Native media embedding | Proposed candidates only | Validate mpv/VLC + shell integration early; avoid a late rewrite caused by untested composition. |
| TV durable database/assets | Unproven | Choose supported persistence after actual evidence; do not hide it behind an always-on server. |
| Representative collection formats | Not recorded | Need real/synthetic representative streams and subtitle/audio requirements before broad compatibility claims. |
| Metadata/artwork retention | Incomplete provider terms | Central offline promise cannot depend on invented cache rights. |
| Subtitle/marker free eligibility | Time-sensitive/conditional | Quotas, retention, attribution and edition accuracy need verification and fallbacks. |
| Dflix ISP restriction | Unverified | Check exact CDN/media endpoints from relevant networks; retain unknown status until then. |
| Nationwide source directory | Desired, not compiled/verified | Needs provenance, verification strategy, maintenance and capability distinctions. |
| Public/commercial distribution | Undecided | No mandatory commercialization/store work, but don't create preventable license/security problems. |
| Name/palette/fonts/final visuals | Unapproved | “Screenroom” and current palette are proposals, not settled brand. |
| Core-versus-advanced release split | Assistant recommendation | User requested all desired features; staged scope must be transparent and agreed when consequential. |
| Exact “Astro” model | Not established | No product dependency; do not confuse it with frontend framework choice. |

### 10.2 Context completeness and unavailable material

This document preserves all substantive project information available from the visible user conversation, carried-forward working context, inspected prototype/draft files and the retrieved handoff preferences. Some earlier assistant turns are not present verbatim in the supplied conversation. No full earlier research transcript or interactive questionnaire implementation was available to reproduce byte-for-byte. Therefore, do not claim this is a literal transcript or an exhaustive source-code embedding. It is a comprehensive project-state handoff with explicit provenance limits.

Where earlier full text is unavailable, preserve the supported result and its uncertainty rather than fabricating exact prompts, unrecorded rejected frameworks, benchmarks, approvals or research outcomes. Any receiving-chat memory from unrelated projects is not permission to alter this project's latest instructions. In particular, unrelated orchestration/model workflows were not imported.

No secrets, tokens, credentials, device serials, signing keys or copyrighted movie files belong in this handoff. Future research should use current authoritative documentation for changing platform/API/terms questions. This transfer itself introduces no new research findings or implementation.

### 10.3 Precedence and reconciliation

The user's newest direct instructions override older aspirations and assistant drafts. Explicit questionnaire answers remain authoritative unless changed. Inspected artifacts establish actual bytes and recorded tests; they do not override stated product goals. Proposed architecture/design choices remain revisable on evidence. A new chat must acknowledge inaccessible attachments or missing repository state rather than assume shared access to this old environment.

## 11. REPO STATE

**Future requirement:** one monorepo root for the entire application and its SDLC artifacts.

**Actual inspected transfer state:**

- Work workspace: `/workspace/scratch/c944d824f37c/`.
- Prototype source directory: `/workspace/scratch/c944d824f37c/screenroom/`.
- Original ZIP: `/workspace/scratch/c944d824f37c/screenroom-v0.1-foundation.zip`.
- Incomplete planning-draft directory: `/workspace/scratch/c944d824f37c/screenroom-handoff/`.
- This transfer document: `/workspace/scratch/c944d824f37c/SCREENROOM_CHATGPT_HANDOFF.md`.
- `git rev-parse --show-toplevel` failed with “not a git repository” for the workspace, prototype and draft directories. **Branch: none established. HEAD: none established. Git working-tree status: not applicable. Remote/PR/commit history: none established in this environment.**
- No existing remote repository was identified, created, pushed or deployed. No merge, commit, worktree or submodule operation occurred during this transfer.
- Existing prototype source and the original ZIP were left unchanged during handoff creation. The partial draft files are not a finished monorepo and are not contained in the prototype ZIP.
- This workspace's filesystem paths do not transfer automatically into normal ChatGPT or the user's Codex CLI environment. Use the actual attached Markdown/ZIP, then inspect the real destination folder when development eventually starts.

**Resume point:** normal ChatGPT, research/planning mode, with this document plus the original ZIP attached. Preserve the decisions above, continue the discussion, and prepare the eventual planning handoff before coding. No current request authorizes building a replacement prototype in that new chat.

# HORCRUX CONTEXT HANDOFF — Offline media application (Screenroom, working title)
