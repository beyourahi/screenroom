# Screenroom — agent instructions

**CURRENT, SUPERSEDING GOVERNANCE (2026-10-27):** Screenroom is the approved
product name, the only project root is `~/Desktop/projects/screenroom`, and the
product is **Screenroom Personal Edition**: one person's own media on that
person's own three devices.
[`docs/ADR-002_RUST_FIRST_STACK.md`](docs/ADR-002_RUST_FIRST_STACK.md) is the
authoritative stack decision, amended by
[`docs/ADR-003_DESKTOP_AND_TV_SCOPE.md`](docs/ADR-003_DESKTOP_AND_TV_SCOPE.md)
and then by [`docs/ADR-004_PERSONAL_EDITION_SCOPE.md`](docs/ADR-004_PERSONAL_EDITION_SCOPE.md),
which is the current word on **which platforms exist**.

**Scope: exactly three targets.** (1) the owner's MacBook Air M4 on macOS,
(2) the owner's Windows ultrawide desktop with an Intel Arc B580, (3) the owner's
Samsung UA50AU8000RXER television. Linux, LG webOS, Android TV, phones and
tablets, Apple TV, Roku, UNiFFI, the Flutter fallback, public distribution and
commercial infrastructure are out of scope (ADR-004).

Rust-first: shared Rust core, iced + wgpu + libmpv on desktop, LightningJS 3 +
Blits + TypeScript with Tizen AVPlay playback on the TV (WebGL/Canvas rendering),
SQLite where supported, FFmpeg/FFprobe for media work, Dropout Design System for
visuals, Cargo + pnpm + the Tizen Studio CLI for builds, MIT for original
first-party code.

Older statements are **SUPERSEDED** and are history, not requirements:
“Flutter is the selected stack” (ADR-001), “Svelte/Tauri is the baseline”,
“Screenroom is a backup name”, “Samsung TV cannot run `.tpk`”, “the UI must be a
WebView”, every iOS/Android phone or tablet requirement (ADR-003), and the
ADR-003 platform list that included Linux, LG webOS and Android TV (ADR-004). Do not act on them. Do not delete them: the archive is the audit trail.

Keep one monorepo, offline-first/zero-paid constraints and every hardware gate.
Never claim a test, a device result or a rendering receipt that did not happen.

## Operating boundaries

**Installed application contract.** Every target is an installed local app: the
desktop executable (`apps/desktop`) or a locally signed Tizen `.wgt`. Never a
hosted website, PWA or browser tab. The owner must not need a browser, internet,
an always-on server or a localhost HTTP server for normal playback, browsing,
resume or lists.

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

The desktop integration is already built and works like this:

- `apps/desktop/examples/export_ds_tokens.rs` reads the private DS at its pinned
  commit and writes a flat Rust pin to `apps/desktop/src/design/ds_pinned.rs`.
- That generated file is **Git-ignored**; only the exporter and the neutral
  fallback in `apps/desktop/src/design/mod.rs` are committed.
- `apps/desktop/build.rs` puts the file on the include path when it is present
  and otherwise exposes the fallback, so a clean checkout builds without the DS
  and a machine with the DS gets the real tokens.
- The active theme string (`dropout-design-system@<commit> semantic.dark`) is
  visible in the running application, so a screenshot shows which pin was used.

See [`docs/DROPOUT_DESIGN_INTEGRATION.md`](docs/DROPOUT_DESIGN_INTEGRATION.md).

**Name.** Screenroom is finalized. Historical reference material keeps the old
labels; active code and docs use Screenroom.

## Repository layout

- `crates/screenroom-core` — Rust domain, SQLite library, scanning, filename
  interpretation, fingerprints. No UI, no network, no platform player.
- `crates/screenroom-player` — libmpv runtime boundary and the player state
  machine. libmpv is loaded at runtime, never linked into Git.
- `apps/desktop` — iced + wgpu desktop client (Windows/Linux/macOS), including the
  libmpv → wgpu presentation bridge.
- `apps/tv` — the LightningJS 3 + Blits client for the owner's Samsung Tizen TV
  only (not started; webOS and Android TV were retired by ADR-004). TV clients do
  not run the Rust core; they follow the documented core contract.
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
  Windows, macOS, Linux, Tizen, webOS or Android TV receipts. A mock or a browser-only test is
  not a device test.
- Verify version-sensitive work against current official docs for the *actual*
  selected release, and respect the lockfiles. Current provisional pins: iced
  0.14 / wgpu 27 (must match iced), Lightning 3 / Blits 2.x, Rust edition
  2024.
- Security: parameterized arguments (no shell interpolation of filenames),
  validated IPC/bindings, strict media-root authorization and canonicalization,
  bounded parser and file inputs, safe URL allowlists, no credentials in
  Git/logs/backups/exports. No destructive cleanup on scan misses or disconnects.
- The private Dropout Design System source and generated private token values
  stay out of Git and out of any public artifact.

## Testing without taking over the desktop

**Background-first, always.** Development and verification run on the owner's own
MacBook while they are using it. A test may not bring Screenroom to the front,
switch Spaces, move the owner's active window, or take the keyboard and mouse —
not even briefly, and not "just to check something".

In order of preference:

1. **Headless.** `cargo test --workspace` and the player examples never open a
   window. Prefer a unit, contract or integration test here. New coverage should
   default to this tier.
2. **Hidden window.** Where a check genuinely needs the running application's own
   platform machinery — AppKit's menu bar, the event loop, the real database —
   launch with `SCREENROOM_WINDOW=hidden`, or a check mode that implies it, and
   must never call `activate`. `scripts/check-quit-routing.sh` is the model: real
   process, hidden window, outcome written to a file, exit. Verify with
   `SCREENROOM_ALLOW_FOREGROUND=0` that the frontmost application does not change.
3. **Programmatic test interfaces.** When a behaviour currently needs a keystroke
   and a visible window, prefer replacing that with a deterministic entry point
   (an environment-gated check, an assertion over the database or the published
   frame) over automating the visible UI. Keep the assertions at least as strong
   as the interaction they replace.
4. **Isolated graphical session.** Only for what genuinely needs a visible window,
   and only after checking that a supported isolated environment is available. Do
   not add heavyweight virtual machines, do not change global macOS settings, and
   do not disturb unrelated applications.

**Foreground UI automation is opt-in.** `scripts/drive-app.sh`, the raise in
`scripts/capture-window.sh` and `scripts/verify-quit.sh` are gated on
`SCREENROOM_ALLOW_FOREGROUND=1` through `scripts/foreground-guard.sh`, and refuse
otherwise. Do not set that variable on the owner's behalf. When a check needs a
visible window and permission has not been given, record it as **PENDING** with
its exact steps and consolidate it into a later owner-approved session — never as
passed, and never quietly skipped.

Capturing a window (`screencapture -l <window>`) is passive and does not need
permission; only activating the application does.

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
   lifecycle/focus and durable storage/artwork. The certificate profile is created
   locally with the owner's own Samsung account; no package or key material is
   committed.
4. **Concurrency and durability** — interruption, power loss, low storage and
   stale player callbacks on the real device.

Retired gates: LG webOS, Android TV (ADR-004) and iOS (ADR-003). Do not reopen one
without a new ADR.

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

Evidence helpers (`scripts/`, all local, nothing published):

```bash
scripts/capture-window.sh OUT.png            # Screenshot of Screenroom's own window
scripts/drive-app.sh library|enter|space|left|right|capture NAME
scripts/capture-window.swift                  # Window-number resolver behind the above
cargo run -p screenroom-core --example prepare_library -- LIB.DB MEDIA_ROOT
```

Capture the **window number**, never a screen rectangle: a rectangle records
whatever is on top of it, so on a multi-display desktop it can silently contain a
different application — including the owner's own viewing. If a capture is not
Screenroom's UI, delete it; never keep it as evidence.

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
