---
title: "libmpv event ids and a Dock-launched app's PATH both silently broke playback facts"
author: Beyourahi
date: 2026-10-29
branch: main
cost: { minutes: 1, tokens: 1922 }
id: l-261d9e13
scope: repo
status: active
src: agent
touches:
  - crates/screenroom-player/src/lib.rs
  - apps/desktop/src/artwork.rs
  - crates/screenroom-player/src/worker.rs
---

## Decided
- mpv event ids are taken from /opt/homebrew/include/mpv/client.h, never from memory: START_FILE=6, END_FILE=7, FILE_LOADED=8, VIDEO_RECONFIG=17, SEEK=20, PLAYBACK_RESTART=21, PROPERTY_CHANGE=22. The old constants were wrong, so seek confirmation and display-size updates never fired.
- A resume or seek counts as done only after PLAYBACK_RESTART for the same playlist entry. The desktop stores progress only when a snapshot matches the current `load_id`, has `position_ready` set, and has a `seek_serial` at least the one it asked for. Without this, a fresh load reports position 0 and overwrites the resume point.
- Anamorphic sizing comes from `dwidth`/`dheight` on VIDEO_RECONFIG. `width`/`height` are the coded size, which gives 1920x1080 with bars for a 2:1 SAR 9:8 file.
- ffmpeg and ffprobe are resolved by `artwork::tool`: PATH first, then /opt/homebrew/bin and /usr/local/bin, absolute directories only.

## Watch out
- An app launched from the Dock or Finder gets the launchd PATH (/usr/bin:/bin:/usr/sbin:/sbin), so `Command::new("ffmpeg")` fails and the failure is silent: no posters, no runtimes. Test the bundle under `env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin`.
- macOS tempdirs live under /var, which is a symlink to /private/var. Fixtures that compare stored root paths must canonicalize the tempdir first.
- A Codex worker in a sandbox cannot run cargo while the shared heavy turn is busy, and cannot write turn.sh markers. Treat its commits as uncompiled until they are built here.
