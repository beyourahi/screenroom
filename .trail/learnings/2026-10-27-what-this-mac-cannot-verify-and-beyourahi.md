---
title: "What this Mac cannot verify, and why"
author: Beyourahi
date: 2026-10-27
branch: main
id: l-e8e9a870
scope: repo
status: active
src: agent
touches:
  - crates/screenroom-core/src/library/mod.rs
  - docs/TEST_PLAN.md
---

## Decided
Path case-folding moved from `cfg!(any(windows, target_vendor = "apple"))` to a
value, `PathSemantics`, settable with `Library::with_path_semantics`. Case folding
is a property of the *volume*, not the OS — a case-sensitive APFS or NTFS volume
exists — and a value is the only shape that lets both branches be tested from one
host.

## Tried and ruled out
- **Cross-checking Windows from this Mac.** `cargo check --target
  x86_64-pc-windows-msvc` fails inside `libsqlite3-sys`: the bundled SQLite needs
  the MSVC SDK headers (`fatal error: 'stdlib.h' file not found`, via
  `/Library/Developer/CommandLineTools/.../mm_malloc.h`). It is a toolchain gap,
  not a code failure, and it means every `cfg!(windows)` branch — the libmpv
  candidate names, the D3D11 bridge choice — is unbuilt as well as unrun. Do not
  read a green macOS suite as Windows coverage.
- **Asserting the case-sensitive *grant* path.** This host's volume folds case, so
  `Media/SHOWS` canonicalises to the same directory as `Media/Shows` and the
  second grant is refused by containment, not by case. No amount of test-writing
  reaches that branch here; the test detects the volume and asserts the reachable
  half instead.

## Watch out
`add_source` canonicalises *before* it keys a path, so two spellings of one folder
collapse on a folding volume and do not collapse on a case-sensitive one. Any test
here has to canonicalise before comparing keys, and any test that skips a branch
has to say so in the test rather than pass quietly.
