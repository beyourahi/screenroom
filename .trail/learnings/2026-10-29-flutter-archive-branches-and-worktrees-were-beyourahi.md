---
title: Flutter archive branches and worktrees were deleted on 2026-10-29
author: Beyourahi
date: 2026-10-29
branch: main
cost: { minutes: 11, tokens: 174682 }
id: l-2af57b15
scope: repo
status: active
src: agent
touches:
  - AGENTS.md
  - README.md
  - docs/ADR-002_RUST_FIRST_STACK.md
  - docs/ADR-003_DESKTOP_AND_TV_SCOPE.md
  - docs/ARCHITECTURE.md
  - docs/IMPLEMENTATION_STATUS.md
  - docs/NAMING.md
  - docs/REUSE_LEDGER.md
  - CLAUDE.md
---

## Decided
The owner chose to delete the local-only `archive/*-2026-10-25` branches and `.worktrees/` (Flutter candidate) with no backup during /consolidate. Docs (AGENTS.md, ARCHITECTURE, IMPLEMENTATION_STATUS, NAMING, REUSE_LEDGER, ADR-002/003 amendments) were updated to say so.

## Tried and ruled out
Pushing archive/* to GitHub as the audit trail, or leaving them in place: both declined by the owner.

## Watch out
The "Deliberately not ported yet" Dart APIs in docs/IMPLEMENTATION_STATUS.md (metadata updates, relink, backup export/import, jobs, NFO parsing, authorizedPath) have no source left; reimplement from the documented behaviour. Paths like `.worktrees/*/packages/...` in REUSE_LEDGER.md are provenance only.
