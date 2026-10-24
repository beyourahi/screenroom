**FINAL OWNER DECISION: SCREENROOM.** Former alternatives in this file are historical, not pending choices. See ADR-001.

> **2026-10-25 Screenroom decision supersedes outdated technology/name claims:** See [`ADR-001_SELECTED_STACK.md`](ADR-001_SELECTED_STACK.md) for the authoritative **Flutter + native platform media + SQLite** stack and `~/Desktop/projects/screenroom` path. Samsung 2021 officially supports Flutter `.tpk` apps; older “`.wgt` only” statements are WRONG and kept below only as historical proposal text. Dropout Design System remains the canonical visual spec, mapped to Flutter; its Svelte components cannot be used directly in Flutter. Windows/Tizen hardware performance and media features are NOT tested.

# Naming exploration — one-word brand, selection pending

**Decision:** final product name is **not chosen**. `Screenroom` is a **backup only**. Existing package folder `screenroom/` is a temporary historic workspace label, NOT an approved name. Do not modify identifiers in builds/installer/data directories until owner picks a brand.

**Naming meaning:** a collection of *your own local* films and shows, presented like a premium streaming catalog; playback, resume and next episode. The name must not imply we host or sell a preloaded catalog or automatically acquire copyrighted content.

| Name | Wordplay / why it fits | Tradeoff |
|---|---|---|
| **Reelcase** | A *case of film reels*: personal catalog, beautifully showcased and ready to play | Emphasizes library above auto-resume |
| **Respool** | Re-spool a film reel: return to where you stopped; rewatch or resume | More metaphorical; meaning needs one sentence |
| **Reelkeep** | Keep your reels: locally sourced, private and durable media collection | Less playback-specific |
| **Nextup** | Continue Watching and autoplay next episode | Less obviously locally owned movies |
| **Playcase** | Play + showcase: organize your own titles and watch | Less explicit cinema reference |
| **Reelshelf** | Your personal movie/series shelf | Focuses on storage, not playing |
| **Reelbase** | Film reel + local media database | More technical |
| **Flickcase** | Flick = film; case = personal collection | More casual |
| **Playback** | Exactly what this application does | Generic and crowded |
| **Rewind** | Continue and return to your movies | Existing multiple video products |

**Recommended initial finalists:** **Reelcase**, **Respool**, **Nextup**. All are proposed, not approved, and *none* has received complete trademark, handle, domain or app-store clearance.

**Known preliminary collisions, not an exhaustive search:** `Spool` is an existing TV watchlist/history app; `FilmDeck` is a movie/TV tracker; `Playdeck` is a professional video playout app; `Reprise` is a native music player; `FlickVault` and `Rewind` also have existing video-related uses. These names should not be presented as clear/unique without investigation. Search absence is not trademark clearance.

**After owner selection:** trademark/app-store/domain/handle check; then update name/logo/installer/app IDs and technical root labels carefully across *one monorepo*. Preserve the original handoff/ZIP as immutable historical material. Logo/wordmark follow Dropout DS, not the old Screenroom prototype style. No automatic public registration or expense is authorized.
