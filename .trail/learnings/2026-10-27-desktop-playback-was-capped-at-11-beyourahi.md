---
title: "Desktop playback was capped at 11.5 fps by the frame packer, not libmpv"
author: Beyourahi
date: 2026-10-27
branch: main
id: l-47b409b4
scope: repo
status: active
src: agent
touches:
  - crates/screenroom-player/src/render.rs
  - crates/screenroom-player/src/worker.rs
  - apps/desktop/src/video_surface.rs
---

The desktop software bridge presented 11.5 fps against a 30 fps source with the
playhead at 1.00x, i.e. frames were being DROPPED, not decoded slowly.

First guess was wrong: `WAIT_SLICE = 0.02` in the worker loop looked like the
throttle, because a new frame is announced through libmpv's RENDER CALLBACK and
not its event queue, so blocking a full slice before re-checking the callback
couples presentation to event arrival. A `WAIT_SLICE_PLAYING = 0.004` was added,
measured, and made no difference at all — it was reverted. Do not re-try it.

The real cause was in `render.rs`: the pack step copied the frame pixel by pixel
(per-pixel push plus alpha write) over 2,073,600 pixels, costing ~40 ms/frame,
while `mpv_render_context_render` itself cost ~3 ms. Replacing the pixel loop
with a chunked copy-and-stamp took the bridge to 30.0 fps with zero drops, in
both debug and release.

Two things worth keeping:
- Instrument by SPLITTING the cost (time libmpv's render vs. your own pack step)
  before changing either. Guessing cost a full edit-measure-revert cycle.
- The alpha channel was previously "repaired" because the presentation shader
  sampled it. It is now written opaque during the pack, so the guarantee lives in
  one place instead of being assumed by the shader.

Measured after the fix (same clip, 1080p): release build 120 frames / 4.00 s =
30.0 fps, playhead 1.00x, ~66-76% of one core and a flat ~255 MB RSS during
playback falling to ~3% / 108 MB after the clip ends.
