# ADR-004 — Screenroom Personal Edition (amends ADR-002 and ADR-003)

- **Status:** Accepted
- **Date:** 2026-10-27
- **Amends:** [ADR-002](ADR-002_RUST_FIRST_STACK.md) and
  [ADR-003](ADR-003_DESKTOP_AND_TV_SCOPE.md). Where either enumerates supported
  platforms, the platform list below replaces that enumeration. Everything else in
  both ADRs — the Rust-first split, the iced + wgpu + libmpv desktop stack, the
  LightningJS 3 + Blits TV stack, the Dropout Design System rules, the evidence
  standard — still applies.
- **Owner decision:** the owner fixed the product scope. It is not a proposal.

## Decision

The product is **Screenroom Personal Edition**: one person's own media, on that
person's own three devices. It is a private, installed application, not a product
that ships to anyone else.

| # | Target | Hardware | UI | Playback |
| --- | --- | --- | --- | --- |
| 1 | Owner's MacBook Air M4 (`macOS`, arm64) | Apple M4, Retina + external 3440×1440 | Rust + iced + wgpu | libmpv, software bridge today |
| 2 | Owner's Windows desktop (`Windows`, x86_64) | ultrawide display, **Intel Arc B580** | Rust + iced + wgpu | libmpv; Arc hardware decode is the target |
| 3 | Owner's Samsung TV (**UA50AU8000RXER**) | 2021 Crystal UHD, Tizen 6.0 | LightningJS 3 + Blits + TypeScript | Tizen AVPlay |

Exactly these three. Nothing else is a target.

## Out of scope

Permanently removed from the current scope, not deferred:

- **Linux desktop.** ADR-003 listed it; the owner has no Linux machine to run it on
  and no receipt could ever be produced for one.
- **LG webOS.** No webOS device is owned.
- **Android TV / Google TV.** No Android TV device is owned, and the Android SDK,
  JDK and Gradle are not part of this project's toolchain.
- **iOS, Android phone and tablet apps**, and the UniFFI layer that existed only to
  serve them ([ADR-003](ADR-003_DESKTOP_AND_TV_SCOPE.md) already removed these).
- **Apple TV, Roku** and any other TV platform.
- **Public distribution and commercial infrastructure.** No store submission
  (Samsung Seller Office, Microsoft Store, Mac App Store), no public release
  artifacts, no notarized distribution build, no hosted service, no crash or
  analytics reporting, no accounts, no payments, no telemetry, no update feed.
  The application is installed by the owner, for the owner.
- **Cross-device sync.** Per-device state stays independent, as ADR-002 requires.

## What this changes in practice

- **`apps/tv` is the Samsung Tizen client only.** Its sole packaging target is a
  signed `.wgt` for Tizen 6.0. There is no webOS `.ipk` and no Android TV APK.
- **Storage is SQLite everywhere it matters.** The TV's storage adapter is the only
  place a non-SQLite representation may be needed, and it must keep the same
  entities and invariants.
- **macOS signing stays out of scope.** Apple Command Line Tools are enough to
  build and run the Rust application; there is no notarized `.app` bundle to
  produce because there is no distribution.
- **Samsung signing is still required** — a Tizen `.wgt` cannot be installed
  without a certificate profile — but it is a **local developer-profile** step on
  the owner's own Mac, performed with the owner's Samsung account. No `.tpk`/`.wgt`
  and no certificate material enters Git.
- **The Linux line in the toolchain docs is gone.** Cargo on macOS and Windows,
  pnpm plus the Tizen Studio CLI, FFmpeg/FFprobe. Nothing else.
- **Windows is not a port.** `apps/desktop` is one crate that must build and run on
  both target operating systems; the libmpv → wgpu bridge is per-platform work
  (macOS today; D3D11/shared-texture path for Windows and Arc), not a second app.

## Gates

Gate numbers are stable so older receipts stay unambiguous. G1–G3 and G5 keep their
meaning; the others are retired rather than renumbered.

| Gate | Meaning | State |
| --- | --- | --- |
| G1 | Workspace builds, tests, lints, formats on the dev host | goal met |
| G2 | libmpv client API on the dev host | goal met |
| G3 | A libmpv frame presented through wgpu in an iced window | goal met on macOS: 30.0 fps at 1080p, correct aspect, transport and resume verified in the window; Windows D3D11 path still BLOCKED |
| G3b | Audio output and audio/video sync | NOT RUN (not heard; no tolerance measured) |
| G4 | macOS **and Windows/Arc B580** playback quality on owner hardware | Windows half BLOCKED (no hardware access) |
| G5 | Samsung UA50AU8000RXER: signed `.wgt`, Lightning 3, USB, AVPlay, focus, durability | BLOCKED (device, certificate) |
| G6 | LG webOS and "other TVs" | **RETIRED** by this ADR |
| G7 | iOS | retired by ADR-003 |
| G8 | Android TV / Media3 | **RETIRED** by this ADR |
| G9 | Durability and interruption on real devices | macOS NOT RUN; Windows and Samsung BLOCKED |

See [`FEASIBILITY_GATES.md`](FEASIBILITY_GATES.md) for conditions and receipts.

## Consequences

- Fewer platforms means fewer unverifiable claims. Windows and the Samsung TV stay
  **mandatory** milestones even while they are BLOCKED, because the owner owns both
  devices and intends to run Screenroom on them.
- The capability matrix, PRD platform statements, architecture, test plan and
  gates were reconciled on this date to match this ADR.
- No code was deleted by this decision except the platform enumerations in the
  documents: `apps/tv` and the Android/Linux work had not been started.

## Exit criteria

Revisit only on an owner decision. Adding any further platform — or distributing to
anyone — needs a new ADR, because it would change what can be verified and what the
project is licensed to do.
