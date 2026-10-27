---
title: "Windows would have refused every healthy source, and nothing on the Mac could see it"
author: Beyourahi
date: 2026-10-27
branch: main
id: l-006a245e
scope: repo
status: active
src: agent
touches:
  - crates/screenroom-core/src/paths.rs
  - crates/screenroom-core/src/scanner.rs
  - crates/screenroom-player/src/lib.rs
  - apps/desktop/src/app.rs
---

The Windows gate is blocked because `cargo check --target x86_64-pc-windows-msvc`
cannot compile at all from macOS: `libsqlite3-sys` builds bundled SQLite from C and
that needs the MSVC SDK headers (`fatal error: 'stdlib.h' file not found`). The
consequence is stronger than "untested": EVERY `cfg!(windows)` branch is unbuilt as
well as unrun, so reasoning about the platform's documented rules is the only
verification available. Do not spend time trying to make the cross-toolchain appear
functional — it is a toolchain boundary, not a code failure.

Three real Windows defects were found that way and fixed with Windows-shaped inputs
asserted from macOS:

- `std::fs::canonicalize` returns a VERBATIM path on Windows, so `canonicalize(r"D:\Media")`
  is `\\?\D:\Media` (and a UNC share comes back as `\\?\UNC\server\share`). The
  scanner's symlink guard compared that against the stored root literally, so it
  refused every healthy Windows source as a replaced symbolic link. Identity now
  goes through `paths::location_key`.
- `D:\Media` did not compare equal to `D:/Media`, so the same folder could be granted
  twice. Both separators are one separator on Windows.
- `libloading` 0.9 calls `LoadLibraryExW(name, 0, 0)`, and flags 0 make Windows search
  the CURRENT WORKING DIRECTORY for a bare name. A packaged app also could not find a
  DLL beside its exe, so the bundled paths were tried first on Windows only.

The pattern worth keeping: a path comparison that is correct on POSIX is not
"probably fine" on Windows. Two of the three defects were lexical comparisons, and
neither produced an error anyone could see on a Mac. The rules are now values
(`PathStyle`, `PathSemantics`) rather than `cfg!`s precisely so the other platform's
behaviour can be exercised from here — that is what makes `with_path_style` and
`with_path_semantics` worth their existence.

One thing that is deliberately NOT fixed: a Windows volume GUID path
(`\\?\Volume{…}\Media`) stays distinct from `D:\Media`. They name the same folder and
no string function can know that, so a USB drive returning under a different letter
still needs an explicit relink. Do not "fix" it by guessing.

`docs/WINDOWS_VALIDATION.md` is the handoff: prerequisites, exact commands, per-area
acceptance checks, and the check that would falsify each of the four fixes.
