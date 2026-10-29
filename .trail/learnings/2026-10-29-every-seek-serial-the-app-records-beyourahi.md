---
title: "Every seek serial the app records must be settled by the worker, or checkpoints stop for the whole viewing"
author: Beyourahi
date: 2026-10-29
branch: main
cost: { minutes: 10, tokens: 88879 }
id: l-9159e745
scope: repo
status: active
src: agent
touches:
  - crates/screenroom-player/src/worker.rs
  - apps/desktop/src/app.rs
---

## Decided
- The app stores progress only when snapshot.seek_serial >= the serial it last sent (snapshot_is_current). So the worker must settle every serial it receives: an early seek (before FILE_LOADED) is queued and runs on load; a NaN or negative seek, or a failed seek command, is confirmed at once; END_FILE for the current entry settles any pending or queued seek, because a seek into the end gets no PLAYBACK_RESTART.
- A requested pause is remembered in the worker (set_paused), because pause changes before START_FILE are not observed.
- Any viewing that ends (finish_active) drops the up-next offer, so a replay during the countdown is not replaced.

## Watch out
- Adding a new early return to Worker::seek without calling confirm_seek or queueing it silently disables saving for the rest of that viewing. No error is shown; the old resume point just stays.
