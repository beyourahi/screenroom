> **2026-10-25 Screenroom decision supersedes outdated technology/name claims:** See [`ADR-001_SELECTED_STACK.md`](ADR-001_SELECTED_STACK.md) for the authoritative **Flutter + native platform media + SQLite** stack and `~/Desktop/projects/screenroom` path. Samsung 2021 officially supports Flutter `.tpk` apps; older “`.wgt` only” statements are WRONG and kept below only as historical proposal text. Dropout Design System remains the canonical visual spec, mapped to Flutter; its Svelte components cannot be used directly in Flutter. Windows/Tizen hardware performance and media features are NOT tested.

# Implementation roadmap and delivery gates

**Updated non-negotiables (2026-10-25):** deliver *installed local applications*, not a website/PWA; the Samsung installed `.wgt` runs an embedded web UI plus native AVPlay; desktop Tauri is a local OS-WebView shell with native Rust/media (must be measured). UI styling and primitives derive from the private Dropout DS through reviewed immutable commit vendoring. Name is TBD, Screenroom backup only. Make M0 include native-app/DS-M76 compatibility/performance gates; avoid investing in visual polish atop a failing playback composition.

**Planning only; no development was executed for this package.** A precise schedule/calendar deadline has not been requested. All client projects remain **in the same root**. Stage scopes are recommended sequencing, not a user-approved cut of desired functionality. Full list F01–F25 remains tracked in the PRD and capability matrix.

## Execution principles

- **Start in plan mode in Codex CLI:** inspect current local project and this package; propose minimum viable experiments, identify device access and providers; only implement after the owner initiates development.
- **No premature multi-platform scaffolding:** create actual apps/packages/crates when a gate requires them, with a thin working end-to-end outcome and tests.
- **Prove G1/G2 early:** the riskiest decisions are desktop native video+UI composition and TV USB+AVPlay+durable storage. Building months of visual features before hardware proof risks costly rewrite.
- Every stage ends with `Completed / Verified / Blocked / Next`, exact platforms/codecs, no false pass for mocks. Checkpoint Git history, secrets ignored, local reproducible commands.
- Primary devices are peer priorities. Don't build full Windows app before discovering TV limitations; TV can proceed in parallel within monorepo if resources permit.
- Heavy work and exploratory code are isolated in disposable feature branches/worktrees **within the same Git repo** (not separate product repos), and only stable validated work is merged by owner-approved workflow.

## M0 — Technical feasibility (no polish required)

**Goal:** determine whether proposed stack can satisfy the actual hardware and strict-free/offline constraints.

Deliverables: environmental diagnostics (non-sensitive), free toolchain setup, signed physical Tizen dev package test, USB enumeration and AVPlay local media test, Windows Tauri/libmpv video composition test, Tizen `wgt-private` data/artwork persistence test, source-URI/codec manifest, clarified TMDB caching policy/fallback. Each experiment is kept as small verifiable source/tests inside the monorepo, but no giant UI implementation.

- **G1** desktop libmpv+Tauri video surface/controls/fullscreen/resize/track/hw decode on actual ultrawide.
- **G2/G3** TV signed app+USB URI+AVPlay+remote+offline restart/store proof; document simulator mismatch.
- **G5** metadata/artwork offline retention license/policy clearance or lawful local/offline alternative.
- **Gate:** either satisfactory physical evidence or explicit BLOCKED/FAIL with tested option/fallback. A technical design choice that fails must change via ADR before M1.

## M1 — Minimum **real** two-device vertical slice

**Goal:** evening of viewing on primary devices, using generated/permitted files and **no internet** after setup. This is a feature milestone, not full product.

Deliverables: root workspaces, stable typed domain/player/source contracts, native desktop and Tizen adapters, one authorized folder/USB scan, known/unknown title row, real playback seek/stop/tracks within hardware scope, profile-aware session-safe progress, Continue Watching and TV deterministic Back/focus. Durable local persistence and event-race protections from day one; mocked APIs only for unit tests, never represented as real platform success.

Acceptance: actual Windows and TV test clips played with correct aspect, progress resume after app restart/power-cycle with network off, disconnected source state non-destructive, unsupported sample yields useful message, no source modification. If TV physically blocked, deliver Windows slice **and** a clearly blocked TV report without claiming M1 two-device completion.

## M2 — Complete offline-first core

**Goal:** fill F01–F13 and F20–F22 with honest platform limitations.

Workstreams: robust incremental scanning/relink/fingerprint/error handling; work/season/episode/edition identity and manual overrides; provider-eligible all-collection enrichment with queue/retry; permission-compliant actual local posters/backdrops; metadata offline retention/attribution; profile/watchlist/state migration/backup; subtitle/audio controls and Unicode; next-episode handling including missing/special episodes; 10k-record searchable/virtualized library; TV focus/accessibility and resource budgets.

Acceptance: scripted offline power/relaunch; unplug/relink and renames preserve watched/overrides; actual artwork bytes survive validated platform lifecycle; entire dataset reachable; provider outage/invalid data harmless; runtime/edition progress not misapplied.

## M3 — Desired advanced features, never silently omitted

**Goal:** F14–F19 and F23–F25 where feasible, permitted, and useful.

- Skip intro/credits: manual segment controls and validated optional IntroDB integration without bulk harvesting, with per-edition runtime checks.
- Subtitle download: user-triggered account/API handling, rate limit/eligibility and content permission; save authorized results, or fall back to local subtitle file.
- Local recommendations: explainable ranking from available collection and per-profile history with sparse-data fallback.
- Browse previews: subtle muted focus-delayed one-decoder-per-device, low-end/reduced-motion still fallback; test that it doesn't interfere with video playback.
- PiP/casting: only genuine device-supported paths, explicit opt-in LAN session and receiver discovery; no hosted service. Describe unsupported cases rather than creating nonfunctional UI.
- Bangladesh directory/acquisition: provenance, permission, actual-source connection checks and source-specific open/search/download/stream; begin with manual/source bookmarks. Dflix CDN/ISP restrictions and TorrentBD details remain unverified.
- Desktop conversion: ffprobe→remux/audio fix/full transcode selected by codec mismatch, safe separate output, estimates/quotas, cancel/cleanup, validated output and correct source/edition lineage. TV client offers appropriate computer-side explanation, not remote execution without owner consent.

Gate: per feature/platform status `VERIFIED / PARTIAL / BLOCKED / NOT_SUPPORTED_BY_PLATFORM`. When an advanced requirement proves impossible or legally restricted in personal free mode, record permitted substitute and ask owner to accept the limitation rather than deleting the requirement.

## M4 — Personal distribution readiness and operational hardening

- Validated builds/install: reproducible Windows installer and signed Tizen **development** package; external native dependencies and FFmpeg/mpv license inventory; no paid app store publishing.
- Migrations: backup/restore, database corruption, TV app update vs dev reinstall, stalled jobs, crash/power loss, media drive disconnect, low storage, source permission revocation.
- Resource/perf: cold/warm launch, browse responsiveness and real video formats on owner's gear, long-running 4K/audio playback and remote controls; choose measured thresholds based on M0 baseline, no invented universal benchmark.
- Security/privacy: IPC/file-root/URL/credentials/import fuzz-like tests, no private media in build evidence/Git, audit rate limits and network disclosures.
- Product closure: every F01–F25 row is implemented/verified or expressly listed as unsupported/deferred with owner acceptance; known issues linked to reproduction. A "personal release" requires actual two-device evidence, not just a browser preview.

## M5 — Optional other platforms when chosen

Mac via desktop adapter with independent libmpv/WebView tests; then potential Linux/Android/iOS/other TV vendors through the **same repo** and appropriate OS-specific adapters. Avoid paying Apple/store fees in the strict $0 personal phase. Reassess battery, platform media permissions, native bundle and hardware decoders; don't pretend universal "runs anywhere."

## Backlog and evidence management

Create issue/task IDs by requirement and platform, e.g. `F07-WIN-video-surface`, `F07-TIZEN-usb-avplay`, `F06-TIZEN-binary-cache`; associate failing tests and ADR. Keep source handling/metadata/rights tasks visible even if blocked. Separate unit, simulated, browser and on-device verification explicitly. Update docs and capability matrix whenever architecture changes. Avoid redundant docs/complex planning automation if a test and ADR explain the decision.

## Risks / mitigations summary

1. **TV file and media URI:** hard gate, signed real install, absolute path + AVPlay proof first.
2. **Native video/UI layering:** experiment before finalizing player UI; alternative libVLC/native surface if needed.
3. **Artwork licensing/offline:** provider permissions verification, local-artwork alternate.
4. **TV data loss:** private files, app upgrade/reinstall tests and independent backups.
5. **User messy files:** explicit identity/collision/manual correction, source disconnected state.
6. **Large library / low-end TV:** virtual scroll/cursor queries/capped preview, measurable input responsiveness.
7. **No paid infra:** no hidden subscriptions; document free API quotas and zero-cost fallback.
