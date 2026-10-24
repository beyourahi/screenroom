# Screenroom — planning baseline v1.2 (25 October 2026)

**DECISION UPDATE:** The product name is **Screenroom**, the destination is **`~/Desktop/projects/screenroom`**, and the selected installed-app stack is **Flutter/Dart UI + per-platform native playback + SQLite**, not the older provisional Svelte/Tauri-first or Tizen-`.wgt`-only direction. Read `docs/ADR-001_SELECTED_STACK.md` **FIRST**; its technology decisions supersede conflicting passages below. Other product requirements remain unchanged. No actual hardware deployment has occurred.

# Local media application — SDLC planning baseline (revised 25 October 2026)

**Status: planning deliverable, NOT an implemented app or an approved final design.** The final one-word product name is **UNDECIDED**; "Screenroom" is a backup/legacy codename only. This folder is intended to become **one Git monorepo** containing all first-party code for all platforms, tests, documentation, and design. There are no active applications in this package yet.

## Read before coding

1. `AGENTS.md` — permanent engineering rules and evidence standards.
2. `docs/PRD.md` — user requirements, release scope, F01–F25.
3. `docs/DECISIONS_AND_RESEARCH.md` — confirmed/proposed/unresolved decisions and current source links.
4. `docs/FEASIBILITY_GATES.md` — nonnegotiable **real-device** playback/storage experiments that must precede a major implementation.
5. `docs/ARCHITECTURE.md` and `docs/DATA_MODEL.md` — provisional platform boundaries, domains, persistence.
6. `docs/DESIGN.md` and `docs/DROPOUT_DESIGN_INTEGRATION.md` — confirmed Dropout Design System ownership and platform-adapted cinematic UX.
7. `docs/NATIVE_APP_STRATEGY.md` — installed-native application contract and web-engine limitations.
8. `docs/NAMING.md` — short, meaningful one-word candidates; final name not chosen.
7. `docs/IMPLEMENTATION_PLAN.md`, `docs/CAPABILITY_MATRIX.md`, `docs/TEST_PLAN.md` — milestone sequencing, capabilities and verification.
8. `docs/PROTOTYPE_REVIEW.md` — what can be salvaged from the early prototype.
9. `docs/CODEX_KICKOFF.md` — ready-to-paste, model-agnostic initial development brief (use when ready to start in Codex CLI).

## Source of truth and precedence

**2026-10-25 user corrections:** (1) the private **Dropout Studio Design System** is mandatory for first-party UI foundations and visual language, not the prototype colors; (2) deliver **installed on-device applications** for target platforms, not a website/PWA/browser-tab product, with platform-native media/FS services; (3) prioritize measured performance, premium cinema-like UX and offline operation; (4) rename only after owner selects a name. Samsung's installed TV application necessarily uses its embedded Tizen web engine; Tauri desktop also uses an embedded OS WebView. Those facts must never be concealed or labeled a fully native UI toolkit. See the two dedicated integration/strategy documents. The historical `screenroom/` directory in this planning ZIP is a temporary workspace identifier, **not final branding**.

1. Newest direct user instruction.
2. User's confirmed selections in `reference/SCREENROOM_CHATGPT_HANDOFF.md`.
3. Requirements in this package, labeled **confirmed** or **proposed**.
4. Primary source documentation and **actual reproducible hardware tests**.
5. Prototype code, **only** as historical implementation evidence.

Conflicts are recorded in a decision log; do not erase contrary evidence. Actual device behavior can invalidate an architectural proposal, but not silently repeal user requirements.

## Hard constraints

- Exactly **one root directory and Git repository** for all first-party clients/code/assets/tests/docs, no submodules or external client repos. OS app-data, SDK installations, dependencies/cache, binaries and the user's actual videos live outside Git when appropriate.
- Initial personal project requires **no paid services, cloud backend, login, account, permanent media server, or cross-device sync**.
- Videos are read where stored; **never delete, move, rename or overwrite source media** without separate user instruction. Explicit conversion creates separate outputs.
- Primary hardware: Windows ultrawide PC and Samsung `UA50AU8000RXER` (2021 AU8000/Tizen 6.0 family). Mac optional; iPhone later optional.
- All F01–F25 features remain tracked. Some are advanced and gated by supported APIs/provider terms; **do not pretend a prototype implements them**.
- No borrowed Netflix/Dflix trademarks/media or unauthorized content sourcing; network directories/bookmarks never imply provider permission or a right to download.
- Development/design happens later through Codex CLI; this package has been researched and authored **without executing the prototype or testing physical devices**.

## Recommended working order

**M0 evidence gates → M1 vertical slice on Windows AND Samsung → M2 durable library/offline experience → M3 advanced capabilities → M4 release/recovery hardening → M5 optional platform expansion.** This is proposed sequencing, not permission to abandon advanced features. M0 may change the stack if evidence warrants it.

## Repository outline (proposal)

```text
screenroom/
  AGENTS.md  README.md  package.json  Cargo.toml  .gitignore
  apps/desktop/  apps/tv-tizen/  tests/preview/      # preview only for internal QA; never a shipped website
  packages/ui/ packages/domain/ packages/contracts/
  vendor/dropout-design-system/manifest.json (once approved and vendored)
  packages/tizen-token-compat/ packages/adapters-tizen/   # derived DS compatibility, not new token system
  crates/media-core/ crates/desktop-adapter/
  tests/unit/ tests/contracts/ tests/integration/ tests/e2e/ tests/fixtures/
  docs/  design/  tooling/  reference/
```

Package/workspace tooling is deliberately not scaffolded yet: choose what is needed during implementation. Treat `reference/prototype-v0.1.zip` as **unapproved archived material** (SHA-256 `e441d273f1d729ab61e7a1cb6924683d52e528c296a6d027afe892abdaf316a3`), not active source. Compiled `dist`, `tv-build`, `tv-dist`, and generated `preview.html` are not production foundation components. The original 2026-10-24 handoff is preserved verbatim in `reference/`.

## What the user ultimately expects

A genuinely usable, fast, aesthetic offline media library/player on the owner's real Windows/ultrawide and Samsung TV hardware. Choose a media location, scan, see correct title/episode grouping and **permitted saved artwork**, play with correct audio/subtitles/aspect ratio, stop/restart offline, and resume; a missing USB/permission, unknown title, unsupported codec, crash or wrong metadata match must not destroy state. Advanced features must eventually be implemented where feasible or visibly documented with an honest device-specific fallback.
