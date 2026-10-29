---
title: Automatic seeks must wait for a settled snapshot of the current load
author: Beyourahi
date: 2026-10-29
branch: main
cost: { minutes: 1, tokens: 1554 }
id: l-ebc64ff0
scope: repo
status: active
src: agent
touches:
  - apps/desktop/src/app.rs
---

## Decided
- Until the resume point lands, the player reports position 0 or the previous file's position. Early seeks are now queued, not dropped, so any automatic seek driven by position (auto-skip-intro) must first check snapshot_is_current, or an intro marked from 0:00 replaces the resume point.

## Watch out
- A new automatic action added to poll() (skip credits, chapter jumps) needs the same gate.
