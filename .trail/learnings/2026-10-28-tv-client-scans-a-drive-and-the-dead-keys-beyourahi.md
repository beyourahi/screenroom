---
title: "TV client: a drive is scanned now, and the dead remote-key trap"
author: Beyourahi
date: 2026-10-28
branch: main
id: l-4f1c7e2a
scope: repo
status: active
src: agent
touches:
  - apps/tv/src/core/scan.ts
  - apps/tv/src/core/name.ts
  - apps/tv/src/core/tracks.ts
  - apps/tv/src/core/settings.ts
  - apps/tv/src/app/runtime.ts
  - apps/tv/src/platform/scan.ts
  - apps/tv/src/screens/Player.ts
  - apps/tv/test/screens.test.ts
  - apps/tv/test/scan-pipeline.test.ts
---

# The TV client scans a drive; the remote reaches the screens

**Date:** 2026-10-28 · **Area:** `apps/tv` (Samsung Tizen), gate G5

## The trap that had already shipped

The first build of the TV client put `left()`, `up()`, `enter()` and `info()` in each
screen's **`methods`** block. Nothing routes a key there. Blits delivers a key only to a
component whose **`input: {}`** block names it, and it fails *silently*: the event
arrives, no handler is found, and the press is discarded. So on a real remote every
arrow key, Enter and Back would have done nothing, while 82 unit tests passed.

Two consequences worth remembering:

- **Delegation, not relocation.** The keys are declared in `input` and call the method;
  the logic stays where it was.
- **A wiring test is worth having.** `test/screens.test.ts` reads each screen's source
  and fails when a screen stops declaring a key it implements. It cannot prove the TV
  delivers the key — that is a device gate — but it does catch the class of bug that
  looks like "the remote is broken".

The Player claims Back itself (`$router.back()`), because a Back press must close the
track menu before it leaves the file. Every other screen leaves Back to the router,
which has its own `input: { back }` on `RouterView`.

## Where the scan lives, and why

`app/runtime.ts`, not a screen: a walk of a USB drive outlives whatever started it, and
a scan owned by a component is abandoned the moment the owner presses Back. The split
that made it testable is the same one the rest of the client uses:

- `core/scan.ts` — every decision (classification, the two bounds, identity across a
  rescan, and the rule that **only a complete walk may withdraw availability**).
- `platform/scan.ts` — a queue, three platform calls, yielding every 20 folders, and
  cancellation checked between folders.
- `platform/media-roots.ts` — `listStorages`, `listDirectory`, `isDirectory`,
  `pathExists`, `toURI` and the storage-state listener.

## Two rules that were easy to get wrong

1. **A drive that leaves is not a drive that is deleted.** A pulled USB marks its rows
   `available: false` and keeps every position. Nothing is ever removed, so plugging it
   back in restores the library without a rescan (a scan is still needed to know which
   files are *there*).
2. **Only a scannable storage becomes a source.** `reconcileSources` will happily grant
   a source for every mounted storage the platform reports, and the TV reports its own
   internal flash and virtual roots (`images`, `documents`, …). Walking those would
   catalogue the platform's bookkeeping. The integration test caught this: it asserted
   one source for one drive and found two.

## The test that is worth its weight

`test/scan-pipeline.test.ts` installs a fake `globalThis.tizen.filesystem` — the
*documented* shape: `listStorages` reports label/type/state, `listDirectory` reports
names — and then runs the production `scanDrives()` end to end: drives → walk → filename
parsing → catalogue plan → **one** stored document → the accessors the screens read.
Ten tests cover a rescan keeping every id, a deleted file keeping its position, a pulled
drive, a drive that will not list, an internal storage, and a host with no filesystem
API at all.

It proves the wiring. It proves nothing about USB permissions, timing, or AVPlay on a
2021 set, and it is recorded that way — a simulated platform cannot fail the way a
television fails.

## What moved out of "not built"

Scanning, external sidecar subtitles (`.smi`/`.srt`/`.sub`/`.ttxt`, the four Samsung
documents), the audio/subtitle menu (`setSilentSubtitle` is what "off" means, not a
track index), remembered per-device preferences, and the verified next-episode offer
(one candidate one number up in the same season, or nothing — a gap is never skipped).

## Still unverified, and why the handoff says so

The television could not be reached: `remote-device-cli scan` swept
`192.168.0.0/24` on SDB port 26101 and found no device, `sdb devices` listed nothing,
and Device Manager had no saved address to retry. So G5 is still BLOCKED, and the
package hash, not the device, is what this session can hand over.
