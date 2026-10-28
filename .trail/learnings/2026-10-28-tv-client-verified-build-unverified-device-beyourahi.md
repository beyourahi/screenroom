---
title: "TV client: verified build, unverified device, and three AVPlay traps"
author: Beyourahi
date: 2026-10-28
branch: main
id: l-19a350a1
scope: repo
status: active
src: agent
touches:
  - README.md
  - docs/ARCHITECTURE.md
  - docs/CAPABILITY_MATRIX.md
  - docs/FEASIBILITY_GATES.md
  - docs/IMPLEMENTATION_STATUS.md
  - docs/TEST_PLAN.md
---

# The TV client's build is verified; the TV is not

**Date:** 2026-10-28 · **Area:** `apps/tv` (Samsung Tizen), gate G5

## What was decided

`apps/tv` is a **second implementation of the core contract**, not a build of the Rust
core. The TV has no Rust, so the client ports `domain.rs`'s entities and re-implements
the rules that *decide* behaviour as pure functions with tests: `core/focus.ts` (the
D-pad rule, index-for-index the desktop's), `core/progress.ts` (the
session/generation/sequence guard), `core/store.ts` (the durability rules),
`core/locator.ts` (relative-locator validation). The platform is behind ports
(`StoragePort`, a playback port) so it is replaceable and the untestable surface is as
small as it can be made.

## The three things that would otherwise have cost a device session

1. **AVPlay's `setDisplayRect` is in a fixed 1920×1080 space**, whatever the app
   resolution is. A rect handed over in app pixels lands somewhere else, or off screen.
   `platform/avplay-pure.ts` converts, with tests, including a degenerate viewport
   (a zero or unmeasured size) that must not produce `NaN`.
2. **`getTotalTrackInfo()` must be read per entry.** Samsung's own published sample
   reads `totalTrackInfo.type` where it means `totalTrackInfo[i].type`, so it inspects
   the array and reports an empty menu. Read one at a time, skip unusable entries,
   parse `extra_info` defensively.
3. **Back is `10009` and is not in the engine's default keymap.** Without
   `keymap: { 10009: 'back' }` the most-pressed key on the remote does nothing. The
   media keys must additionally be *requested* with `tvinputdevice.registerKey`.

## The trap avoided

`Math.max(1, Math.trunc(NaN))` is `NaN`, so "sanitise a count" by flooring and clamping
does not sanitise it. A grid built from an unmeasured layout produced `NaN` card widths
and would have placed the whole grid nowhere, with nothing to report. Every count in
`design/cards.ts` now goes through a finite check.

## The build shape, and why it is not the desktop's

One bundle (`inlineDynamicImports`, fixed `main.js` name), relative base, `es2019`
target for the 2021 engine — a widget has no server to fetch a second chunk from and no
origin for an absolute URL to resolve against. `scripts/prepare-tizen-dist.mjs` stages
the widget and **refuses** to continue if the bundle is unlinked, missing, or referenced
absolutely, because each of those failures is a blank screen on a TV with no console.

The widget is staged at `build/widget` rather than `build/tizen` on purpose: the Tizen
build writes the `.wgt` *inside* the project directory it is given, so staging and
packaging into the same path meant re-staging deleted the one artifact that takes a
toolchain to make.

## The rule this session had to keep re-applying

A clean typecheck, a green off-device suite, a single-file bundle and a signed `.wgt`
whose payload hashes match the tested one are all **build** facts. None of them is a
device fact, and the only authority on the engine, the renderer, AVPlay and the remote
is the television itself. So the client's device handoff names, per open question, the
observable that settles it — because the failure mode to avoid is a build that is
finished and a gate that reads as passed.

The one measurement worth carrying forward: the Tizen build runner's envelope reports
whether a build fell back to the SDK's default developer certificates. That warning,
not the presence of a `.wgt`, is what distinguishes a package that can reach a real TV
from one that only reaches an emulator.

## Also not built yet

The TV has no scan: a drive can be enumerated through the platform API but not walked,
so nothing is catalogued there and the screens show their (tested) empty states. No
external subtitle files, and no track menu. Recorded as gaps rather than half-built,
because a menu that cannot be shown to work is worse than no menu.
