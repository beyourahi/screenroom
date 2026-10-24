> **2026-10-25 Screenroom decision supersedes outdated technology/name claims:** See [`ADR-001_SELECTED_STACK.md`](ADR-001_SELECTED_STACK.md) for the authoritative **Flutter + native platform media + SQLite** stack and `~/Desktop/projects/screenroom` path. Samsung 2021 officially supports Flutter `.tpk` apps; older “`.wgt` only” statements are WRONG and kept below only as historical proposal text. Dropout Design System remains the canonical visual spec, mapped to Flutter; its Svelte components cannot be used directly in Flutter. Windows/Tizen hardware performance and media features are NOT tested.

# Platform capability and scope matrix

**Shell/UX source of truth (confirmed, untested):** Windows must launch as installed desktop application with local OS WebView + native player candidate; Samsung as installed signed `.wgt` with native AVPlay; optional Mac as installed desktop application. No website/PWA/required web-server products. Canonical Dropout DS semantic components/token system on first-party UI with validated TV M76 adapter (none yet validated). Real hardware performance remains unknown. Do not confuse installed app with fully native-rendered widgets. See `NATIVE_APP_STRATEGY.md` and `DROPOUT_DESIGN_INTEGRATION.md`.

**Legend:** `C` = proposed core requirement, `A` = desired advanced capability, `?` = feasibility unverified, `—` = not intended/needed now. **None of these statuses means implemented or physical-device tested.** Future clients must report verified runtime capability at content/OS level. Existing v0.1 browser code is historical only.

| Feature | Windows ultrawide | Samsung 2021 TV | Mac future | Notes / fallback |
|---|---|---|---|---|
| F01 Source choose/relink | C? | C? | A? | TV USB Filesystem API; confirm actual mounts. |
| F02 Incremental index/scan | C? | C? | A? | Nonblocking; TV resource limits. |
| F03 Work/episode/edition grouping | C | C | A | Shared domain; same file may serve episodes. |
| F04 Auto match + override | C | C | A | Unknown remains visible, offline. |
| F05 All-collection enrichment | C | C | A | Optional network, durable queued sync without cross-device state. |
| F06 Offline artwork bytes | C? | C? | A? | Provider rights and TV persistent store gate. |
| F07 Broad player | C? | C? | A? | Windows native libmpv candidate, TV AVPlay decoder. |
| F08 Local profiles/progress | C | C? | A | No account required. |
| F09 Continue Watching first | C | C | A | Persist across offline restart. |
| F10 Gap-safe next episode | C | C | A | Use verified episode order. |
| F11 Search/filter/sort 10k | C | C? | A | Cursor/virtualized; resource test on TV. |
| F12 Watchlists/favorites | C | C | A | Profile-specific local. |
| F13 Tracks/subtitles | C? | C? | A? | TV external format support especially limited; user-visible fallback. |
| F14 Intro/credits skip | A | A? | A | Only validated edition markers/manual. |
| F15 Subtitle provider download | A? | A? | A? | Free optional service/quotas/retention terms. |
| F16 Explainable recommendations | A | A? | A | Only user's owned/connected collection. |
| F17 Muted browse preview | A? | A? | A? | Resource/reduced-motion fallback to stills. |
| F18 Picture-in-picture | A? | A? | A? | Actual OS/TV supported path; mini-window distinctly labeled. |
| F19 Casting | A? | A? | A? | Authorized opt-in LAN + receiver compatibility; no permanent server. |
| F20 Backup/recovery | C | C? | A | TV uninstall/dev reinstall may erase app data. |
| F21 Settings/provider secrets | C | C? | A | TV secret-storage API unknown; no credential leak. |
| F22 Unavailable states | C | C | A | Preserve disconnected source/history. |
| F23 BD ISP/FTP directory | A? | A? | A? | Verified provenance and current-connection status. |
| F24 Legal acquisition options | A? | A? | A? | External bookmark first, integrations only if permitted. |
| F25 Local conversion | A? | — | A? | Computer-only FFmpeg; TV can recommend computer conversion. |

## Acceptance state to update for each platform/feature

Each row needs `{planned, implemented, unit_verified, browser_verified, on_device_verified, limitations, linked_test_receipt, reason_if_blocked}`. Report `?` as **UNKNOWN**, not TRUE. Add Android/Linux/other TV clients only when formally targeted; keep contracts reusable, not empty new apps.

### Device-specific guarantees to avoid

- `.mkv` or `.mp4` support does not imply every codec/profile/audio/subtitle combination. Samsung 2021 lacks DTS; H.264/HEVC Main10 limits depend on specifics.
- TV app 1920×1080 UI coordinate space does not imply a 1080p video cap on 4K panel, nor guarantee HDR passthrough.
- Browser playback screenshots do not prove libmpv hardware decoding, AVPlay USB URIs, local images surviving uninstall or accessible casting receivers.
- "PiP" on a desktop, Samsung TV and iPhone may require different platform support; avoid shared false affordances.
- Offline movie libraries on separate devices do not mean USB mounted on Windows is visible on Samsung. A streaming server is not implicit.
