> **2026-10-25 Screenroom decision supersedes outdated technology/name claims:** See [`ADR-001_SELECTED_STACK.md`](ADR-001_SELECTED_STACK.md) for the authoritative **Flutter + native platform media + SQLite** stack and `~/Desktop/projects/screenroom` path. Samsung 2021 officially supports Flutter `.tpk` apps; older “`.wgt` only” statements are WRONG and kept below only as historical proposal text. Dropout Design System remains the canonical visual spec, mapped to Flutter; its Svelte components cannot be used directly in Flutter. Windows/Tizen hardware performance and media features are NOT tested.

# Feasibility gates — prove hardware before major implementation

**2026-10-25 scope update:** treat the installed-app contract and Dropout DS compatibility as hard gates in addition to media/TV persistence. Installed `.wgt` on Samsung is a web-engine UI by official platform restriction, *not a website or an all-native executable*. `NATIVE_APP_STRATEGY.md` and `DROPOUT_DESIGN_INTEGRATION.md` define evidence gates for Tauri OS WebView overhead, Dropout private source adoption, M76 token/CSS compilation, D-pad focus, UI responsiveness, and offline packaged assets. If incompatible, document an alternative adapter or propose a justified platform-specific deviation **for explicit owner approval**; never silently ship a different UI.

Status as of 2026-10-24: **all real-device tests NOT RUN**. Documentation-backed feasibility is not implementation proof. Gates use reproducible, rights-cleared sample media; do not automate network scans or test copyrighted titles without permission. Conduct in **single monorepo** when the user later starts Codex CLI development.

## G0 — Environment, sources and zero-cost prerequisites

**Inputs:** inspect actual current repo, Windows OS+CPU/GPU+ultrawide resolution/scaling/audio chain (no serial/IP), TV actual model/firmware and Tizen dev mode, usable FAT32/exFAT/NTFS USB media details if visible, realistic/legal test media, SDK availability, local free storage and certificates. Never presume the existing Mac is a Windows machine; execute Windows observations on Windows, and Samsung observations on the physical TV.

**Pass evidence:** machine info (non-sensitive), complete free toolchain paths/versions, TV signing/test-install method (no hidden paywall), codec fixture inventory and disk free space in an ignored local evidence report. On inaccessible hardware status **BLOCKED—not PASS**.

## G1 — Windows native player + premium UI composition

**Question:** can libmpv (or libVLC) provide genuine broad codec/audio/subtitle playback and controllable video surface with **Tauri 2 + shared Svelte UI** on actual Windows?

**Experiment:** minimal isolated test client in same monorepo; use documented `libmpv` API with tested package provenance. Compare child-window/WID embedding and render-API approach for video under/alongside HTML overlay; try fullscreen, ultrawide sizing, DPI/monitor change, resizing, layered native controls, correct letterboxing, switching tracks, pause/seek/rate, power sleep/relaunch, hardware decode off/auto, HDR if supported by chain, long path/unicode/space filenames. Test 1080p H.264/AAC, 4K HEVC Main10 MKV/MP4, alternative audio and subtitle tracks; record actual media specs and observed CPU/GPU/memory. Test cancellation and release native resources.

**Pass:** reliable first frame and responsive controls with correct aspect, good overlays or an explicitly acceptable native-control alternative; seek/resume checkpoints are actionable with event timings; reproducible packaging/licensing plan; no crashes for basic/repeated open-close cycles.

**Fallback if fail:** compare libVLC; test a separate native player surface/window explicitly integrated into desktop shell; only then reconsider desktop framework. A webview `<video>`-only solution **does not** satisfy broad-format ambition without an accepted limitation.

## G2 — Samsung developer install and actual USB to AVPlay

**Question:** on `UA50AU8000RXER`, will a signed Tizen Web package reach USB files and play them through AVPlay?

**Experiment:** use genuine Tizen 6/M76-compatible static bundle, proper WebAPI script & privileges; sign with Samsung certificate and device authorization; install via the documented TV developer workflow, **not USB sideload**. Use `tizen.filesystem.listStorages`/`resolve`/read-only enumeration; record storage label, mount state, file URI without disclosing personal paths; create AVPlay `object`, call `open` with absolute local file URI, register listener, async prepare, set rect/letterbox, play/seek/pause/stop/close. Test H.264+AAC baseline, HEVC Main10, MKV, and intentionally unsupported DTS-only audio (expect truthful failure, not a crash). Never move or alter originals.

**Pass:** on-screen real playback with audio, remote controls and correct aspect/size; USB removal/reconnect yields unavailable/recover state; developer logs and exact on-device version/codec matrix saved. Mock or emulator pass is **not** G2 pass.

**Fallback:** determine whether Samsung filesystem `toURI`, AVPlay absolute path, permissions, USB filesystem or developer build is failing. Diagnose without assuming API absence; if inherent limit, consider TV native player app-launch handoff (if actual supported) or approved optional computer conversion, and expose restriction. No always-on Windows computer server without new consent.

## G3 — TV persistence and real offline artwork

**Question:** what actually survives relaunch, TV power-off, device restart, **normal app update versus Tizen Studio development reinstall**, and USB disconnect on this TV?

**Experiment:** write versioned sample profile/progress and several locally generated poster binary assets to **app-private persistent storage**, not localStorage. Test `wgt-private` filesystem read/write privilege/storage size, quota, atomic-write/rename or safe journal equivalent; optionally benchmark IndexedDB support from M76 if useful. Simulate interrupted save, corrupt manifest, no free space, two profile states and 100/10,000 library records (sized realistically). Close app, unplug network, power-cycle, reopen. Test normal update semantics and **dev-reinstall erasure** as separately tracked scenarios; export a small backup to user-approved accessible location if feasible.

**Pass:** offline restart restores profiles/progress, actual decoded poster bytes and matches without network; no silent data reset. Document measured storage ceiling and migration/backup path. Samsung documents Web Storage only 5 MB, so a big `localStorage` JSON snapshot is an **automatic fail**.

**Fallback:** durable versioned file store in `wgt-private` rather than arbitrary SQLite/IndexedDB assumptions; external user-authorized backup where possible; constrain artwork quantity and display graceful missing-art fallback. A cloud backend cannot silently replace local persistence.

## G4 — TV remote, subtitles, lifecycle and performance

**Experiment:** deterministic D-pad navigation on deep rows/dialogs/virtualized lists, focus restoration, Back hierarchy, key repeats, on-screen keyboard return, seek/play media keys, remote-only source picker. Test subtitle track selection, local `.smi` and other documented types (do **not** presume arbitrary `.srt` compatibility in AVPlay), Bengali Unicode where possible, subtitle offset. Test app suspend/resume, failed media open/stop and a full evening-like sequence. Observe 10-foot text size/safe margins and input latency; use physical evidence.

**Pass:** no mouse needed, no remote trap, durable progress and correct video/aspect, responsive UI within locally measured acceptable bounds; document unsupported tracks/codec cases and recovery options.

## G5 — Offline enrichment provider and artwork retention

**Question:** can an entirely free personal app legally retain actual third-party movie/TV data and artwork offline in the way the user requires?

**Experiment:** verify latest TMDB terms/FAQ plus any explicit caching/retention policy. Preserve URL/date and any written clarification obtained; test authorized metadata and actual image download with user-managed read token/credit; validate source attribution, artwork size, offline restart, rate limits, revocation and deletion/refresh mechanisms. If terms do not permit required persistence, explore an alternate legally compatible metadata/artwork source and **local poster/embedded media artwork fallback**. Don't scrape a provider to circumvent retention restrictions.

**Pass:** documented permission for exact selected storage/refresh model, **or** a transparent lawful source/local-artwork fallback that truly works offline; provider outage does not break viewing. Provider quota data is not a blanket promise.

## G6 — Network source directory and permitted acquisition

**Experiment (advanced, later):** for any user-confirmed lawful endpoint (including exact `https://cdn.dflix.live` if permissions allow), distinguish URL and protocol, connection scope (DOT/other ISP/VPN), page vs actual media response, login and legal automated-use permission. Validate one endpoint at a time by permission, no arbitrary scans/port enumeration or unauthorized torrent scraping. Preserve current network result/last checked timestamp; offer external bookmark if integration is unsupported.

**Pass:** label each source VERIFIED/LOGIN_REQUIRED/NOT_REACHABLE/NOT_CHECKED, specific capabilities and provenance. Nationwide directory is never falsely asserted without sufficient coverage. This is **not required to deliver the local player**.

## Evidence record template

Each gate result in ignored `.artifacts/gates/Gx/` plus a redacted summary in `docs/DECISIONS_AND_RESEARCH.md`:

- Date + test operator/environment + actual model/OS and installed firmware + tool versions.
- Exact sample specs (container, streams, profiles, resolution, fps, subtitle, duration, checksum); use generated/rights-cleared clips.
- Steps/commands, logs (sanitized), screenshots/screen recording only if authorized.
- Observed results vs expectations, latency/memory/resource numbers, failures and reproductions.
- Outcome **PASS / FAIL / BLOCKED / CONDITIONAL**, actionable fallback, revisited ADR and next gate.

**Stop/go rule:** Do not build a large shared UI or promise broad TV/Windows native playback before G1/G2 can at least demonstrate a real reliable vertical slice. Continue independent planning/testing if TV hardware blocked, but leave TV explicitly uncertified and avoid a release claim. G5 must be resolved before promising permanent third-party offline posters.
