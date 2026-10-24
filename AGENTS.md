# Screenroom — agent instructions

**CURRENT, SUPERSEDING GOVERNANCE (2026-10-25):** Screenroom is the approved product name, the ONLY project root is `~/Desktop/projects/screenroom`, and `docs/ADR-001_SELECTED_STACK.md` is the authoritative installed native-stack decision. Use Flutter/Dart UI, per-device native playback, SQLite, and a faithfully mapped Flutter adapter to the private Dropout Design System. Older statements such as “Screenroom is backup”, “Samsung TV cannot run `.tpk`”, and “Svelte/Tauri is the selected baseline” are explicitly SUPERSEDED. Keep one monorepo, offline/zero-paid constraints and hardware gates. Never claim tests happened without a real receipt.

# Agent instructions — unnamed local-media-app monorepo (Screenroom = backup name)

This is an **AI-operated**, personal, offline-first multimedia product. Human familiarity with a programming language is not a stack-selection criterion. Optimize actual usability, playback quality, portability, performance, operability and maintainability.

## Operating boundaries

**2026-10-25 superseding decision — product shell:** every released target is an **installed local app**, never a hosted website, PWA or browser tab. The interface may use an **embedded native OS WebView**, but describe it accurately as hybrid, not pure native UI; Windows desktop services and video engine remain native, and Samsung TV ships an installed `.wgt` with native AVPlay playback because Samsung TVs do not support native `.tpk`. The end user must not need a browser, internet, an always-on server or a localhost HTTP server for normal operation. `tests/preview/` is strictly developer QA, not an end-user client.

**2026-10-25 superseding decision — visuals:** follow the owner's private **Dropout Studio Design System** as canonical visual/component source. Before frontend changes inspect its actual `DESIGN.md`, `GUIDELINES.md`, `README.md`, canonical DTCG `src/lib/tokens/tokens.json`, component lifecycle and current commit. Apply reviewed, immutable-commit-pinned private vendoring inside this monorepo through the DS's approved sync path; never fetch a public registry mirror, paste an unreviewed copy, or publish private DS source in planning bundles/public packages. Screen-specific cinematic compositions remain in the product. For TV M76, build/validate a documented compatibility presentation adapter using the same canonical semantic tokens; do not silently fork the design language. Read `docs/DROPOUT_DESIGN_INTEGRATION.md` and `docs/NATIVE_APP_STRATEGY.md`. The original pale-green palette is superseded.

**Name:** final one-word name is still undecided. `Screenroom` is a backup historical label, not an approved product brand. Do not automate broad renaming until chosen.

- Keep **every first-party** platform app, package, crate, script, test, design source, decision and development document in this single root/repo. Do not create separate repos, submodules, external shared-project folders or unwritten local dependencies. SDKs, OS runtimes, caches and user videos are external by nature and are not part of Git.
- Never modify, rename, delete, move or overwrite user videos. Scan/read in place and show failures without destructive cleanup. Conversion jobs require explicit user intent and write separately.
- Do not introduce required paid services, SaaS infra, cloud accounts, always-on hosts, AI inference fees or hosted sync. Do not enable automatic purchases, submissions, posts or deployment.
- No actual pirate-site integration, scraping, authentication automation, bypassing access controls, or distribution of protected works without an appropriate permissions/rights check. Preserve user-specified sources as bookmarks/capability research, never assume a public link is permission.
- **Source state ≠ user approval ≠ hardware certification.** Never manufacture actual Samsung/Windows test receipts. Do not claim a feature is complete because it has a button, mock or browser-only test.
- Use current official Svelte/SvelteKit, Tauri, Samsung, FFmpeg, mpv and provider docs when implementing anything version-sensitive. Verify APIs against the actual selected release and respect its lockfile. Do not prescribe an AI model or effort level in prompts.
- Security: parameterized OS subprocess arguments, validated IPC, strict media-root authorization and canonicalization, no arbitrary privileged webviews, safe URL allowlists, bounded parser/file inputs, no credentials in Git/logs/export. No destructive cleanup on scan misses or disconnects.

## Required evidence gates

Read `docs/FEASIBILITY_GATES.md`, `docs/NATIVE_APP_STRATEGY.md`, and `docs/DROPOUT_DESIGN_INTEGRATION.md` first. Determine actual Windows hardware and prove native-player/UI integration. On the **physical Samsung UA50AU8000RXER**, prove signed package installation, USB enumeration, AVPlay local playback, lifecycle/focus and durable state/artwork **before** treating Tizen support as achieved. If the device is inaccessible, mark its gate BLOCKED and work independently, never greenwash it.

## Implementation discipline

1. Check actual repo state and user modifications before edits. Do not overwrite a user's existing project or extract the archived prototype on top of the root.
2. Record design/architecture/provider decisions as ADR entries with status and evidence; update the capability matrix and test evidence when a behavior changes.
3. Build thin vertical slices with runnable tests, stable contracts and typed boundaries. Prioritize source authorization → a real playable file → durable progress on both priority devices.
4. Keep domain IDs independent of file path or metadata-provider IDs; persist state transactionally and migrate without silent resets. Keep device state independent initially.
5. Prove offline behavior with network disabled and the app restarted; persist **permitted actual image bytes**, not just remote URLs.
6. Treat feature capabilities per platform. Unsupported combination → truthful reason and recovery/conversion option. No dead buttons, fake data, false empty results or pretend HDR/format support.
7. Keep runtime workload bounded, queue heavy work outside watching, measure on actual hardware. TV D-pad navigation must be deterministic, accessible and reliably restore focus.
8. Use generated or rights-cleared test media, never commit private viewing content, accounts, tokens, DUIDs, certificates, passwords or personal addresses.
9. Document commands and reproducible results for unit, contract, integration, browser and physical-device checks. Do not mark mock-only tests as physical checks.
10. A release stage is complete only with satisfied acceptance checks or explicit user-accepted exceptions; report implemented/verified/blocked/deferred separately.

## Status format

Give compact: **Completed / Verified / Blocked / Next**. Link evidence when available. When uncertain, state what failed or cannot yet be measured.

<!-- trail:start -->
## Trail — repo context graph

This repo is indexed in `.trail/cache/`: small linked markdown nodes that explain each
system and carry exact file:line spans, kept in sync with the code through git.

For ANY task here — understanding how something works, finding where code lives,
or scoping a change — get context from the graph before grepping or opening
source files. Re-ask freely (it's cheap) and reuse literal identifiers you
already have (symbol, error string, file name) as the query. New to this repo?
Run `trail map` first — a token-budgeted orientation (dir clusters, hubs,
hotspots), no LLM, no key.

- Run `trail ask "<your question>" --source` → ranked nodes with the relevant
  code spans inlined (each hit's ≤8-line crux by default; `--full` for whole
  definitions when the crux isn't enough). Match the tool to the task shape:
  for understanding or editing, the top node IS the answer — cite its
  `covers:` file:line spans and edit straight from `--source`. For
  exhaustive tasks ("every occurrence / every caller of this pattern"), ranked
  results are top-N, not complete — run `trail grep "<literal>"` instead
  (exhaustive over indexed files, grouped by enclosing symbol), falling back
  to raw `grep -rn` only for unindexed files.
- `trail skeleton <file>` → every definition's signature + span, ~10× cheaper
  than reading the file; use it to skim an API surface.
- `trail callers <symbol>` gives precomputed, exact edges — who calls this.
  Add `--direction out` for what it calls, or `--depth N` to walk
  transitively for the full blast radius. For structural questions, skip
  ranking and use this directly.
- Or browse: `.trail/cache/INDEX.md` lists every node; follow the links.
- Monorepos and folders of multiple repos rank fairly across sub-projects —
  hits carry `[scope/]` labels naming which one they're from. Narrow with
  `trail ask "<task>" --in <scope>/` once you know where you're working.

If a returned span is truncated ("+N more lines"), open the file at that exact
range before finalizing. Only open source files when a node genuinely lacks a
needed detail, and then at the exact file:line the node points to — never
re-read whole files.

After big code changes, refresh the graph with `trail build` (deterministic,
no API key, $0).
<!-- trail:end -->
