> **2026-10-25 Screenroom decision supersedes outdated technology/name claims:** See [`ADR-001_SELECTED_STACK.md`](ADR-001_SELECTED_STACK.md) for the authoritative **Flutter + native platform media + SQLite** stack and `~/Desktop/projects/screenroom` path. Samsung 2021 officially supports Flutter `.tpk` apps; older “`.wgt` only” statements are WRONG and kept below only as historical proposal text. Dropout Design System remains the canonical visual spec, mapped to Flutter; its Svelte components cannot be used directly in Flutter. Windows/Tizen hardware performance and media features are NOT tested.

# Local media application — Product Requirements Document (name TBD)

Version 1.0 planning revision · 2026-10-25 · **Not a completed implementation or a full user-approved release contract**.

## 1. Product definition and boundaries

**Vision.** A high-performance, premium, cinematic **installed offline-first application** for locally sourced movies and TV episodes, delivering the organization and convenience of a streaming-service experience around media files the user has authorized. Beyond VLC-like playback, it offers poster-based organization, series/episode context, progress, profiles, discoverability, subtitles, reliable autoplay, local recommendations, and advanced conveniences. It does not itself host a movie subscription service or require uploading personal media to a cloud.

**Confirmed core:** user-selected folders/USB/external disks; local, independent per-device history; home with Continue Watching first; next-episode autoplay and browse previews desired; English metadata default; profiles/favorites/watchlists/search; eventual intro/credits skipping, subtitles/downloads, recommendations, PiP/casting and computer conversion; free-only tools/services for personal use; one monorepo; Windows/ultrawide and Samsung TV first; Mac and iPhone optional. Help discovering available external sources, while showing only permitted and feasible actions. See original handoff in `../reference/SCREENROOM_CHATGPT_HANDOFF.md`.

**Proposals, not ratified:** exact screen composition (not the design system), feature milestone split, seven-hundred-millisecond preview delay, five-second autoplay countdown, a 95% completion heuristic, particular storage engines, provider selections and native player. Treat these as changeable defaults, not the user's explicit requirements.

**Not authorized:** silently enabling paid services, an always-on server, cloud sync, unauthorized downloads, DRM bypass, distribution/app-store registration, source-file mutation, claiming universal hardware support or licensing copyrighted artwork.

## 2. Priority audience, devices and operating model

**New confirmed delivery rule (2026-10-25):** target platforms receive a real installable, on-device app: Windows desktop executable/installer; Samsung signed `.wgt` TV application; future Mac installed bundle and platform apps when prioritized. **No website, PWA, ordinary browser-tab application, required hosted service or required localhost web-server runtime.** A locally installed app is not synonymous with a purely native-rendered UI: current Tauri candidate uses OS WebView for Dropout/Svelte interface, and Samsung TV mandates a packaged web-engine UI. Both must use native media playback adapters wherever platform allows. See `NATIVE_APP_STRATEGY.md`.

**Confirmed design-system rule (2026-10-25):** all first-party visuals derive from the private Dropout Studio Design System—black-first editorial foundations, canonical semantic tokens, components, iconography, accessibility, and motion rules. Screenroom prototype palette is explicitly superseded. The cinematic library/player layouts are domain compositions built on these foundations, adapted and tested for 10-foot TV and Windows ultrawide. See `DROPOUT_DESIGN_INTEGRATION.md`.

**Naming:** Product name intentionally undecided. Screenroom retained only as backup; no final logo, icon, app ID, repository slug or trademark commitment until chosen (`NAMING.md`).

- Initially one owner, potentially multiple **local viewer profiles** per device; profiles are not authenticated accounts or parental controls.
- **Primary**: owner's Windows PC with ultrawide monitor, frequently used for long viewing. Exact OS build/GPU/CPU/display/HDR chain unmeasured.
- **Primary**: Samsung UA50AU8000RXER (2021 AU8000, 4K panel, Tizen 6.0 model-family baseline; firmware reference 2301.1 historical). Local USB on TV, not assumed to see PC drives. Remote-only operation is essential.
- **Optional later**: MacBook Air M4, iPhone 15. Future Android/Linux/other TV brands are ambitions, not initial commitments or universal claims.
- Media remains separately stored on each device or attached disk; no automatic progress sync; no permanent NAS/LAN host assumed.

## 3. Jobs to be done / user journeys

**J1 First evening offline** — launch app, add a local folder/drive through an authorized picker, see scan progress and items even before scan finishes, identify/inspect a show, play with correct letterboxing and selectable supported audio/subtitles, close app, disconnect internet, reopen and resume. Reboot/unplug/reconnect retains accurate library/history.

**J2 Poorly named downloads** — scan messy filenames, classify episodes conservatively; if confidence low, still show as playable Unknown media; optionally enrich entire library when online, surface ambiguity and allow a manual match that subsequent background jobs never override.

**J3 TV couch session** — remotely add/select USB, browse with D-pad, enter a title, play through AVPlay if compatible, seek/choose tracks, exit/reopen and resume. If the codec is incompatible, show reason and a computer-side conversion path rather than falsely claiming it can play.

**J4 Personalization** — switch profiles on one device, continue separate progress/watchlists/favorites, see recommendations only from locally available collection, optionally start next verified consecutive episode without skipping gaps.

**J5 Adding media** — see source directory/bookmarks and possible external open/search/download/stream actions when lawful/available; inability to integrate a source must not block basic playback; automatic scraping/acquisition is not implied.

**J6 Recovering** — external disk absent, wrong poster, renamed file, lost permission, interrupted source scan, crash/power loss, corrupt backup, low free space and unsupported streams all yield actionable non-destructive states.

## 4. Functional register

IDs are persistent across architecture/implementation/tests. **Milestone labels are proposed**, not proof of delivery. Priority: Core = essential for a useful personal vertical slice/complete core; Advanced = desired but subject to capability/terms; Platform = only where applicable. More detail and device targets in `CAPABILITY_MATRIX.md`.

| ID | Priority | User-facing behavior / completion requirement |
|---|---|---|
| **F01** | Core | **Sources:** choose authorized folders/USB/external drives, label, scan, rescan, relink, remove from library without deleting originals, show connection/permission state. |
| **F02** | Core | **Discovery:** incremental nonblocking cancelable/restartable scan; partial-success errors, bounded traversal, no symlink loops/duplicate roots, detect files still downloading. |
| **F03** | Core | **Structured library:** distinct work/movie/series/season/episode/edition/media-file records; unknowns, specials, anime/alternate ordering and combined episode files don't disappear. |
| **F04** | Core | **Identity matching:** filename/folder context, optional local NFO/IDs, confidence and evidence; manual match/unmatch/fix, protected user overrides. |
| **F05** | Core | **Whole-library online enrichment:** all connected items queued, visible/recent items prioritized, rate limits/retry/offline queue durability, TV/season/episode metadata. Never rely on playback-trigger-only matches. |
| **F06** | Core | **Offline artwork/details:** actual permitted local image bytes + metadata retained for offline display, provenance/retention/refresh policy, graceful posterless state. URL caching alone fails. |
| **F07** | Core | **Player:** validated Windows native engine and Samsung AVPlay path; pause/seek/fullscreen/aspect/audio/subtitles as the platform allows; unsupported combinations honestly handled. |
| **F08** | Core | **Profiles/progress:** multiple local independent viewer profiles, per-edition progress/watched state, periodic/pause/seek/stop/suspend checkpoint, session-race protection. |
| **F09** | Core | **Home/resume:** Continue Watching first; library below; resume/restart/mark watched/unwatched/remove with edition/profile fidelity, appropriate empty states. |
| **F10** | Core | **Next episode:** exact verified sequence, cancelable countdown, no silent gaps/ambiguous episodes, handle end of series/cross-season only with known order. |
| **F11** | Core | **Search/filter/sort:** filenames/titles, movie/series, watched, available, genres when known; all results reachable in a large library (virtualized/paged). |
| **F12** | Core | **Watchlists/favorites:** per-profile, persistent across rename/relink/source disconnect/metadata correction. |
| **F13** | Core | **Tracks/subtitles:** embedded, adjacent and manual file support by device; audio track selection; subtitle language, offset/size where supported; Bengali and Unicode; document ASS/image limitations. |
| **F14** | Advanced | **Skip intro/credits:** verified provider or manually entered markers, confidence/runtime/edition validation, reversible/accurate skip, protect post-credit scenes, offline use only if permitted. No invented markers. |
| **F15** | Advanced | **Subtitle search/download:** free permitted provider with match/release/hash/language, quota/error handling, user-triggered save and offline reuse; manual/embedded fallback. |
| **F16** | Advanced | **Local recommendations:** explainable ranking restricted to owned local library, preferences/history/available titles; no paid/hosted AI required; cold-start behavior. |
| **F17** | Advanced | **Browse previews:** muted delayed preview, one active decoder, capped resources, reduced-motion setting, still-image/offline fallback. |
| **F18** | Advanced | **PiP:** authentic supported OS/device behavior or distinctly labeled mini-player where feasible; never pretend TV/desktop PiP are identical. |
| **F19** | Advanced | **Casting:** real receiver detection plus reachable, permitted stream and compatible codec; any temporary opt-in LAN endpoint must be confined, session-scoped and off by default. |
| **F20** | Core | **Backup/recovery:** versioned export/import of metadata, profiles, history, user corrections, settings and optional permitted artwork; transactional validation; clearly not a backup of videos. |
| **F21** | Core | **Settings/secrets:** device/language/playback/accessibility/provider/cache controls; appropriate credential handling/redaction and migration. |
| **F22** | Core | **Unavailable/error states:** missing/disconnected/permission/provider/corrupt/unsupported differentiated; history retained, retry/rescan/relink, useful diagnostics. |
| **F23** | Advanced | **Bangladesh source directory:** provenance, protocol (HTTPS/FTP/torrent index/etc.), operator/access requirements, last verification/current-connection result and source-specific actions. Never imply nationwide verified access. |
| **F24** | Advanced | **Acquisition choices:** authorized open/search/download/stream where feasible; honest external handoff/bookmark fallback; embedded BitTorrent is a separate decision, not a required implication. |
| **F25** | Advanced (desktop) | **Explicit local conversion:** use FFmpeg/ffprobe as authorized, prefer remux or audio-only fixes, separate output, profile/estimate/progress/cancel/verify and preserved original. |

**Capability limitations do not delete feature requests.** An unsupported feature stays tracked with evidence, alternative and owner decision; client may offer a device-appropriate fallback rather than a disabled-but-deceptive affordance.

## 5. Quality and nonfunctional requirements

**Q00 Distribution/native-app UX (confirmed):** the owner never needs to visit a website, launch Chrome/Edge, or run a background server to use the app. Each officially supported device gets a locally installed launchable app with packaged UI/assets and direct OS/platform APIs; online connections are enrichment options only. Tauri and Tizen use internal embedded web rendering by platform design; do not call that pure native UI. Any proposal to replace the embedded UI must preserve DS fidelity, resource targets and target-device support, with an evidence-backed ADR.

**Q00B Canonical design system (confirmed):** use `beyourahi/dropout-design-system` as the private upstream for first-party components/tokens. Maintain approved private immutable SHA vendoring and correct attribution/license policy; no user-visible interface may silently use the prototype theme or a competing generic component library. Compile compatible TV presentation from shared design values, and prove output against the 2021 Samsung engine.

**Q01 Offline:** core library browsing, profiles, stored permitted images, resume and local playback work with network disabled after previous import. No remote fonts, CDNs or hosted runtime required for the UI. Internet is optional for discovery/enrichment/subtitles.

**Q02 Performance:** cold/warm startup, first useful scan result, input-to-response, first video frame and stable 4K playback measured on owner hardware; no asserted zero-lag promise. Measure Tauri/WebView/DS overhead against an actually feasible baseline on the same hardware, including memory, idle CPU, remote-key latency and animation stability. Watching has resource priority; no runaway scans/transcodes; virtualize results; one preview decoder; cap poster resolutions and queues.

**Q03 Fidelity:** preserve aspect ratio by default, honor stream timebase/audio/subtitle selection, accurately checkpoint by session ID, never pretend a codec/container extension guarantees playback/HDR.

**Q04 Portability:** one monorepo; shared domain contracts and adaptable UI; OS-specific player/permission/storage implementations isolated behind explicit feature/capability interfaces. A TV package is not a Tauri binary.

**Q05 Security/privacy:** least privilege, no whole-drive surveillance, user-granted media roots, import/export validation, no source edits, HTTPS/provider policy handling, no credential leakage, no arbitrary shell from filenames, no unsolicited media/title upload. Optional metadata calls disclose searched title/IDs and require clear explanation.

**Q06 Durability:** migrations/transactions/journals with rollback; device unplugged or update must not silently drop profile/library data. Provider images retained only as permitted; TV uninstall may delete private app data, so backup is needed.

**Q07 Accessibility and design quality:** every critical TV flow works via remote, Windows via keyboard/mouse, meaningful accessible names/contrast/reduced motion, subtitle scaling and legibility, deterministic focus/scroll restoration. Dropout DS WCAG 2.2 AA, contrast, 44×44 coarse targets and borderless semantic focus states apply; TV D-pad focus must be couch-distance-visible without unsupported CSS effects.

**Q08 Honest reporting:** implemented ≠ unit-tested ≠ browser-tested ≠ physical Windows-tested ≠ Samsung-certified. Capability matrix updated at each milestone.

## 6. Release boundaries and stages (proposed)

- **M0 — Evidence first:** physical TV signing/USB/AVPlay/storage test; Windows native player/UI/resize/fullscreen test; provider cache/licensing research; sample media inventory. Result can be STOP/CHANGE STACK/BLOCKED.
- **M1 — Two-device vertical slice:** separate working Windows and Samsung clients in one repo, user-authorized files, real playback on both, progress checkpoint + offline restart, remote navigation and genuine error states. No fake catalog.
- **M2 — Complete core:** F01–F13 and F20–F22 to independently verified supported behavior; robust identity/scanning, collection-wide metadata and permitted durable artwork, profiles/history, large library and recoverability.
- **M3 — Advanced personal experiences:** F14–F19 and F23–F25 where feasible and rights-compliant; record explicit tested fallbacks/owner exceptions, don't drop or counterfeit scope.
- **M4 — Packaging and hardening:** migration/backup/security/codec/perf/accessibility audits, Windows installer and signed **development** TV package, verified personal-use installation and recovery.
- **M5 — Optional platform reach:** Mac, phone, Linux/Android/other TVs through the same shared domain/core repo and new adapters after actual demand. No default requirement to enroll in paid stores.

The user wants **the entire application properly built**; milestones are engineering checkpoints, not a claim that M2 equals every requested feature.

## 7. Product acceptance criteria

A. On **real Windows ultrawide**, from a user-selected drive: see partial scan, locate a movie/series, play compatible test files with aspect/audio/subtitle control, stop/kill/relaunch offline and resume within a documented tolerance. Unplayable format yields clear reason/desktop conversion route.

B. On **real Samsung UA50AU8000RXER**, install the signed dev package, pick mounted USB, launch AVPlay for a locally stored playable file, remote seek/Back, restart TV/app offline and recover progress/artwork from on-device durable storage. Confirm the app never assumes Windows USB is visible to TV.

C. For a renamed/moved title, metadata correction or disconnected drive, stable work/profile history remains; manual override does not regress; missing consecutive episode never silently gets skipped.

D. With 10,000 generated library records, the **entire** result set remains discoverable, scanning/background jobs can be stopped, and UI remains usable; empirical performance numbers are recorded, not invented.

E. Test rights-cleared image retention, source/recovery migration, provider outage/quotas, low storage, subtitle Unicode and unsupported audio/HDR; mark all observed device restrictions.

F. Zero required recurring/one-off paid service subscriptions in personal mode; read originals in place; one repository.

G. Launch a real **installed local app** on Windows and Samsung with the web browser closed and internet off (for all functionality that should work offline). Measure real startup/memory/input/playback performance and verify there is no required local or hosted server.

H. Screens use canonical Dropout tokens/components/pattern rules with tested cinematic compositions, ultrawide scaling, TV remote focus, reduced-motion states and M76 CSS/JS compatibility; only product-specific compositions and platform-necessary, documented adapters are local.

Acceptance maps to `TEST_PLAN.md`; quantitative timing/budget targets are *proposed* until hardware baselines exist.

## 8. Open decisions and exclusions

- Unknown actual Windows hardware/OS/resolution/audio/HDR; the user can supply later, or a permitted on-device script can report non-sensitive facts.
- TV installation, USB permission, filesystem and AVPlay lifecycle/storage survive only theoretical documentation today; physical acceptance requires first-person evidence.
- libmpv/native composition approach, compatible distribution builds/licenses are unsettled.
- TMDB metadata/image offline retention terms unresolved; alternative provider or user/local artwork may be necessary if required caching conflicts with permissions.
- OpenSubtitles quota figures conflict across current first-party help pages; implement usage discovery and conservative fallback, not a hardcoded free tier.
- Dflix exact CDN reachability, TorrentBD URL/integration rights and Bangladesh directory accuracy unverified.
- No automatic media torrent downloading, parental restrictions, cross-device sync, third-party paid platform integration or public-store release specified for initial personal build.
- "Screenroom" brand/name, visual tokens and precise advanced-milestone priorities await later review; routine defaults should be implemented sensibly and transparently, not via an exhaustive questionnaire.
