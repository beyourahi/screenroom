---
title: Desktop artwork never started until a navigation
author: Beyourahi
date: 2026-10-28
branch: main
id: l-d0f0ac43
scope: repo
status: active
src: agent
touches:
  - "apps/desktop/src/app.rs#App::new"
  - "apps/desktop/src/app.rs#App::ensure_artwork"
---

App::launch reads the library through App::with_library, and that path calls reload() -> ensure_artwork() while background_work is still false, so no candidate is ever queued at startup. App::new then set background_work = true and returned, so extraction stayed idle until the viewer navigated (any reload re-entered ensure_artwork). Caught by launching the real binary against a prepared library and finding zero artwork rows, not by any test. Fix: call ensure_artwork() once in App::new right after background_work is enabled. A test cannot see this because tests deliberately build with background_work = false; the receipt is the sqlite count after a hidden launch.
