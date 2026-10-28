---
title: "A Tizen widget loads classic scripts, and webapis is a sibling of tizen"
author: Beyourahi
date: 2026-10-28
branch: main
id: l-6c1f4d92
scope: repo
status: active
src: agent
touches:
  - apps/tv/vite.config.ts
  - apps/tv/scripts/prepare-tizen-dist.mjs
  - apps/tv/src/platform/tizen.ts
  - apps/tv/src/platform/env.ts
  - apps/tv/index.html
  - docs/FEASIBILITY_GATES.md
---

# Two facts that only the television could teach

**Date:** 2026-10-28 · **Area:** `apps/tv` (Samsung UA50AU8000, Tizen 6.0, gate G5)

## 1. A widget is a `file://` origin, so `type="module"` never runs

The packaged app installed and launched and showed a black screen. No console, no
screenshot, no inspector on this firmware — the app simply *was* running and drawing
nothing.

Cause: Vite emits `<script type="module" crossorigin src="./main.js">`, and Chromium
refuses module scripts from a `file://` origin because modules are subject to CORS and
a local file has a `null` origin.

Proven both ways without a TV, in the same origin class the widget uses:

| Page | Canvases | Console |
| --- | --- | --- |
| `dist/index.html` (module tag) | **0** | `Access to script at 'file:///…/main.js' from origin 'null' has been blocked by CORS policy` |
| `build/widget/index.html` (fixed) | **1**, 1920×1080 | clean |

The fix has two halves and both are required:

- stage `<script defer src="./main.js">` instead of the module tag (`defer` preserves
  the "runs after the document is parsed" ordering the module had, which matters
  because the mount point is in `<body>`);
- compile `import.meta.url` away. It is a **parse-time** error in a classic script, so
  one occurrence — inside Blits' version probe — turned the whole bundle into the same
  blank screen by a different route.

`prepare-tizen-dist.mjs` now **fails the build** if a module tag or module-only syntax
survives. A build that packages a bundle the TV cannot run is worse than a build that
stops.

## 2. `webapis` is not inside `tizen`

The client read `tizen.webapis.avplay`. `$WEBAPIS/webapis/webapis.js` installs
`window.webapis`, while the device APIs live on `window.tizen` — they are siblings.

The TV made the contradiction undeniable in its own words: `avplay=0 filesystem=1`.
Both of those come from the same platform, so `filesystem` answering while `avplay` did
not was the tell. The consequence would have been silent and serious: `startPlayback`
would have chosen the `<video>` fallback, which cannot open a local file on a TV, so
every "play" would have failed while the app looked healthy.

`webapisGlobal()` in `platform/tizen.ts` is now the single accessor, and
`test/host.test.ts` pins the rule with the TV's own contradiction as a test case.

## 3. The identity API needs its own privilege

`webapis.productinfo` is present but answers nothing without
`http://developer.samsung.com/privilege/productinfo` in `config.xml`. The install
accepts the privilege; without it the boot record reads `model=unknown` on a television
whose model is printed on the box.

What `getVersion()` returns is the **product** version (`3.7`), not the Tizen version
(`6.0`). It is now kept as `productVersion` and shown under its own label, because
printing `3.7` next to "Tizen version" would be exactly the confident false receipt this
project forbids.

## How to see anything at all on this TV

This firmware gives a developer `install`, `launch`, `applist`, `getduid` and `pull` —
no shell worth the name, no screenshot, no `dlog` over TCP, no RWI port (which needs a
debug launch through `app_launcher`, absent here), and no key injection.

So the app reports on itself: `core/evidence.ts` keeps a bounded log in a `localStorage`
key of its own, and `sdb pull` of the widget's data directory reads it back. That is how
`host=tizen avplay=1 … canvases=1 size=1920x1080 mounted=1` became a fact instead of a
hope. It is a receipt for everything except pixels, focus and sound — those still need a
person with the remote.
