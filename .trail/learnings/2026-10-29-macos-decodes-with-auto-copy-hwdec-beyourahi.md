---
title: macOS decodes with auto-copy hwdec because the software render bridge needs CPU frames
author: Beyourahi
date: 2026-10-29
branch: main
cost: { minutes: 2, tokens: 32017 }
id: l-9f40b54c
scope: repo
status: active
src: agent
touches:
  - crates/screenroom-player/src/worker.rs
  - crates/screenroom-player/src/render.rs
---

## Decided
- The desktop renders through libmpv's software render API, so it can only take frames in system memory. Plain `hwdec=videotoolbox` gives GPU surfaces it cannot read; `auto-copy` (VideoToolbox decode, copied back) works and mpv falls back to CPU decode for codecs outside its whitelist.
- Measured on the M4 with real 4K HEVC Main10: CPU about 344% -> 165% and RSS about 737 -> 486 MiB on the hardest file; every sample lower on CPU and memory, picture identical by mean luma. Evidence: crates/screenroom-player/HWDEC_EVIDENCE.md.
- Other platforms stay on `hwdec=no` until measured. `SCREENROOM_HWDEC=no` forces software decode.

## Watch out
- Even with copy-back, a 3240x2160 HEVC file delivered about 19 fps against 23.976 under mixed load: the bottleneck is the CPU RGBA conversion and upload, not decode. Real 4K needs a GPU render bridge, not more decode tuning.
- The worker no longer polls every 20 ms; it sleeps on a wake signal fed by mpv's wakeup callback, render update callback and every command. Any new work in the loop must be triggered by one of those, or it will not run until something else wakes the thread.
