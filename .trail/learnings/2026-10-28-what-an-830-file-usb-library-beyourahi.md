---
title: What an 830-file USB library exposed that fixtures did not
author: Beyourahi
date: 2026-10-28
branch: main
cost: { minutes: 90 }
id: l-ce46ed9c
scope: repo
status: active
src: agent
touches:
  - crates/screenroom-core/src/scanner.rs
  - crates/screenroom-core/src/filename.rs
  - crates/screenroom-player/src/worker.rs
  - apps/desktop/src/app.rs
---

The owner's SSD library (exFAT, 830 files, 18 shows, 291 films) and the generated
fixtures disagree in four ways that matter:

- macOS writes `._Name.mkv` beside every file on exFAT, and those companions keep the
  media extension. The walk skipped hidden *directories* only, so 830 files became 851
  entries with titles like `._12 Angry Men - 1957`. Skipping dot-files silently is the
  fix; reporting them as issues would make every scan "incomplete".
- Episode filenames never carry the show's year — the folder does. Without reading it,
  every series had `year = NULL` and two same-titled shows would merge, because the
  series identity key is `title + year`. Folder years are applied to episodic files
  only: a film in `Duology (2009-2011)` must not borrow 2009.
- `track-list` reports embedded cover art as a video stream (`image=yes`). Listing it
  offered a "second video track" that is one still frame.
- The poster queue is 16 deep and a Browse page holds 24, and nothing refilled the
  queue when an extraction finished — so the last cards stayed blank until a keystroke.
  Refill freed slots for visible works only: a library-wide idle refill is minutes of
  ffmpeg behind the viewer's back.

Watch out: three of the 830 files are truncated MP4s (`moov atom not found`, the
Cloverfield trilogy). The scanner catalogues them and the player refuses them with
`decode_failed` — that is the honest behaviour, not a bug to paper over. And
`cargo test --workspace` can fail `audio_is_consumed_in_real_time_without_drifting`
(5% tolerance) when the desktop playback tests and the core scale test run beside it;
it passes alone, so the flake is load, not sync.
