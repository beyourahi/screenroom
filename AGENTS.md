# Screenroom — agent instructions

**CURRENT, SUPERSEDING GOVERNANCE (2026-10-26):** Screenroom is the approved
product name, the only project root is `~/Desktop/projects/screenroom`, and
[`docs/ADR-002_RUST_FIRST_STACK.md`](docs/ADR-002_RUST_FIRST_STACK.md) is the
authoritative stack decision. Rust-first: shared Rust core, iced + wgpu + libmpv
on desktop, SwiftUI + AVPlayer on iOS, Jetpack Compose + Media3 on Android,
LightningJS 3 + Blits + TypeScript with platform-native playback on TVs, SQLite
where supported, FFmpeg/FFprobe for media work, Dropout Design System for
visuals, MIT for original first-party code, Flutter + Dart only as a recorded
fallback.

Older statements are **SUPERSEDED** and are history, not requirements:
“Flutter is the selected stack” (ADR-001), “Svelte/Tauri is the baseline”,
“Screenroom is a backup name”, “Samsung TV cannot run `.tpk`”, “the UI must be a
WebView”. Do not act on them. Do not delete them: the archive is the audit trail.

Keep one monorepo, offline-first/zero-paid constraints and every hardware gate.
Never claim a test, a device result or a rendering receipt that did not happen.

## Operating boundaries

**Installed application contract.** Every released target is an installed local
app: a desktop executable, an iOS/Android app, or a signed TV package
(`.wgt` for Samsung Tizen, `.ipk` for LG webOS). Never a hosted website, PWA or
browser tab. The user must not need a browser, internet, an always-on server or a
localhost HTTP server for normal playback, browsing, resume or lists.

**Design authority.** Follow the owner's private **Dropout Studio Design System**
as the canonical visual/component source. Before any UI work, inspect its actual
`DESIGN.md`, `GUIDELINES.md`, `README.md`, canonical DTCG
`src/lib/tokens/tokens.json`, component lifecycle and current commit. Apply
reviewed, immutable-commit-pinned private vendoring inside this monorepo through
the DS's approved sync path. Never fetch a public registry mirror, paste an
unreviewed copy, or publish private DS source in planning bundles, public
packages or screenshots. Each platform adapts the same semantic tokens and
documented control behaviour; no platform may silently fork the design language.
The original pale-green prototype palette is superseded.

**Name.** Screenroom is finalized. Historical reference material keeps the old
labels; active code and docs use Screenroom.

## Repository layout

- `crates/screenroom-core` — Rust domain, SQLite library, scanning, filename
  interpretation, fingerprints. No UI, no network, no platform player.
- `crates/screenroom-player` — libmpv runtime boundary and the player state
  machine. libmpv is loaded at runtime, never linked into Git.
- `apps/desktop` — iced + wgpu desktop client (Windows/Linux/macOS), including the
  libmpv → wgpu presentation bridge.
- `apps/ios`, `apps/android`, `apps/tv` — per-platform clients. TV clients do not
  run the Rust core; they follow the documented core contract.
- `docs/` — ADRs, architecture, roadmap, progress ledger, gates, licensing.
- `reference/` — preserved historical prototype and handoff. Read-only source of
  ideas; never the active build.
- `.worktrees/` — archived agent worktrees, Git-ignored but recoverable through
  the `archive/*` branches. Do not delete them.

Keep **every first-party** app, crate, package, script, test, design source,
decision and development document in this single root. No submodules, no sibling
repos, no unwritten local dependencies. SDKs, OS runtimes, caches and user videos
are external and are not part of Git.

## Hard rules

- Never modify, rename, delete, move or overwrite user media. Scan and read in
  place; report failures instead of cleaning up destructively. Conversion jobs
  need explicit user intent and write to separate output.
- No required paid service, SaaS infrastructure, cloud account, always-on host,
  AI inference fee or hosted sync. Never enable automatic purchases, submissions,
  posts or deployment.
- No pirate-site integration, scraping, authentication automation, access-control
  bypass or distribution of protected works without a rights check. A user-listed
  source is a bookmark and a capability question, never an assumed permission.
- **Source state ≠ user approval ≠ hardware certification.** No invented Samsung,
  Windows, iOS, Android, Tizen or webOS receipts. A mock or a browser-only test is
  not a device test.
- Verify version-sensitive work against current official docs for the *actual*
  selected release, and respect the lockfiles. Current provisional pins: iced
  0.14 / wgpu 27 (must match iced), UniFFI 0.32, Lighting 3 / Blits 2.x, Rust
  edition 2024.
- Security: parameterized arguments (no shell interpolation of filenames),
  validated IPC/bindings, strict media-root authorization and canonicalization,
  bounded parser and file inputs, safe URL allowlists, no credentials in
  Git/logs/backups/exports. No destructive cleanup on scan misses or disconnects.
- The private Dropout Design System source and generated private token values
  stay out of Git and out of any public artifact.

## Required evidence gates

Read [`docs/FEASIBILITY_GATES.md`](docs/FEASIBILITY_GATES.md) first. Gates are per
capability and per device, never per framework:

1. **Desktop presentation** — a libmpv frame must be presented through wgpu in an
   iced window on real Windows hardware (and macOS for the OpenGL bridge).
   Compiling the bridge is not presenting a frame.
2. **Desktop playback quality** — hardware decode, 1080p/4K, HDR, audio and
   subtitle switching measured on the owner's GPU.
3. **Samsung Tizen (physical UA50AU8000RXER)** — signed `.wgt` install, Lightning 3
   bundle running on the Tizen 6.0 engine, USB enumeration, AVPlay local playback,
   lifecycle/focus and durable storage/artwork.
4. **iOS / Android** — UniFFI bindings built into the app, AVPlayer/Media3 local
   playback, scoped storage / file access, and background/rotation lifecycle.
5. **Concurrency and durability** — interruption, power loss, low storage and
   stale player callbacks on the real device.

If a device is inaccessible, mark its gate **BLOCKED** and keep working on what is
independently verifiable. Never greenwash a missing gate.

## Implementation discipline

1. Check actual repo state and uncommitted work before editing. Preserve, do not
   overwrite. Archived worktrees stay on their `archive/*` branches.
2. Record design/architecture/provider decisions as dated ADR entries with
   evidence; update the capability matrix and progress ledger when behaviour
   changes.
3. Build thin vertical slices with runnable tests and typed boundaries. Priority
   order: authorized source → real playable file → durable progress → parity on
   each priority device.
4. Domain IDs stay independent of file path and provider ID. Persist
   transactionally and migrate without silent resets. Per-device state is
   independent by design; there is no cross-device sync requirement.
5. Prove offline behaviour with the network disabled and the app restarted.
   Persist **permitted actual image bytes**, not remote URLs.
6. Capabilities are per platform. An unsupported combination gets a truthful
   reason and an achievable recovery path. No dead buttons, fake data, false empty
   results, or pretend HDR/format support.
7. Bound runtime work; queue heavy jobs outside watching; measure on the actual
   device. TV D-pad focus must be deterministic and must restore predictably.
8. Use generated or rights-cleared test media. Never commit private viewing
   content, accounts, tokens, DUIDs, certificates, passwords or addresses.
9. Document reproducible commands and results for unit, contract, integration and
   physical-device checks. Do not label mock-only tests as device checks.
10. A milestone is complete only with satisfied acceptance checks or an explicit
    user-accepted exception. Report **Completed / Verified / Blocked / Next**
    separately, with links to evidence.

## Commands

```bash
cargo test --workspace          # core + player + desktop logic tests
cargo clippy --workspace --all-targets
cargo fmt --all --check
cargo run -p screenroom-player --example capability   # libmpv on this device
cargo run -p screenroom-desktop                       # desktop shell
```

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
