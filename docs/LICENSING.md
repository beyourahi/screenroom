# Licensing and redistribution policy

Checked 2026-10-26 against the resolved dependency graph on this host. This records
evidence and release requirements. It grants no new rights in third-party code,
brands, codecs, media or private design material.

## First-party source

Screenroom's original first-party code is **MIT**, Copyright (c) 2026 Rahi Khan,
per the ROOT `LICENSE` file. Third-party files keep their own notices.

## Resolved Rust dependency graph

`cargo metadata --format-version 1` reports **408 packages** in the desktop
workspace. License expression distribution:

| License expression | Packages |
| --- | --- |
| MIT OR Apache-2.0 | 180 |
| MIT | 110 |
| Apache-2.0 OR MIT | 35 |
| MIT/Apache-2.0 | 16 |
| Apache-2.0 | 14 |
| Zlib OR Apache-2.0 OR MIT | 14 |
| MIT OR Apache-2.0 OR Zlib | 6 |
| Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 6 |
| Unlicense OR MIT | 4 |
| Zlib | 3 |
| BSL-1.0 | 2 |
| ISC | 2 |

Principal direct components:

| Component | Version | License (declared) |
| --- | --- | --- |
| iced / iced_wgpu | 0.14.0 | MIT |
| wgpu / naga | 27.0.1 / 27.0.3 | MIT OR Apache-2.0 |
| rusqlite / libsqlite3-sys | 0.41.0 / 0.39.1 | MIT |
| serde, serde_json, sha2, thiserror, regex | current workspace pins | MIT OR Apache-2.0 |
| uuid | 1.29.0 | Apache-2.0 OR MIT |
| libloading | 0.9.0 | ISC |

Obligations that follow: preserve MIT/BSD/ISC copyright notices, carry the Apache
2.0 notice and patent grant where used, and keep the zlib and BSL-1.0 notices that
apply to two compiled-in components. Apache-2.0 is compatible with an MIT-licensed
product as long as notices are retained; it does not relicense first-party code.

At M4, `uniffi` (currently 0.32.2) joins the graph under **MPL-2.0**, a file-level
copyleft: keep its notices, and if any MPL-covered file is modified, publish that
file's source. It does not reach into Screenroom's separate original code.

## Native media components: source license is not binary clearance

This is the sharpest release constraint in the project.

| Component | Evidence on this host | Distribution status |
| --- | --- | --- |
| Homebrew `mpv 0.41.0` (bottled) / `libmpv.2.dylib`, client API 2.5 | `brew info mpv` → `License: GPL-2.0-or-later AND LGPL-2.1-or-later` | **Development only.** This is a GPL build. Do not bundle or ship it. |
| Homebrew `ffmpeg 9.0.2` | `ffmpeg -version` shows `--enable-gpl --enable-version3` | **Development only.** Invoked as an external tool for local conversion/QA; not copied into Git or an installer. |
| mpv's own build documentation | default build is GPL; the LGPL subset requires explicit configuration | A release must pin an audited, hash-verified **LGPL-only** libmpv/FFmpeg build, or ship under GPL with full corresponding-source obligations. Decision belongs to the release gate, not to a dependency default. |
| libmpv render API surface | OpenGL / D3D11 / software backends only (`include/mpv/render.h`) | No additional license grant; the same binary obligations apply to whatever is embedded. |
| SQLite engine | public domain ([sqlite.org/copyright](https://www.sqlite.org/copyright.html)) | Bundled through `rusqlite`'s `bundled` feature; wrapper notices still apply. |

For every shipped native binary, record: version, hash, upstream source revision,
complete build configuration, dependency licenses, corresponding source, and any
modifications. Provide relinking/replacement rights where the applicable LGPL
version requires them. Do not ship a GPL-linked binary behind MIT-only notices.

Codec patents and platform store terms are separate from open-source copyright.

## TV clients

The Lightning 3 renderer and Blits are MIT-licensed JavaScript packages
(`@lightningjs/core`, `@lightningjs/blits`, declared on npm). The Bundled JavaScript
still carries its dependency notices, and the TV platform SDK/runtime components
(e.g. Samsung AVPlay, webOS media pipeline) have their own vendor terms. An npm
dependency's permissive license is not permission to redistribute a platform SDK.

## Private Dropout Design System

The private DS remains its owner's proprietary material. The approved private
foundations pin is `41406ab436ef2b9c1edcf69e9d4ada4b94d13eec`.

- No MIT grant is inferred for its source, tokens, generated values, assets or any
  derived adapter.
- It stays ignored locally (`/vendor/dropout-design-system/`, `/src/lib/ds/`,
  `**/design/dropout_tokens.g.dart`) and never appears in a public package, issue,
  screenshot bundle or planning archive.
- Each platform maps the same semantic roles through its own presentation layer.
  A public build must not require the private source: the mapping sits behind an
  interface with a neutral fallback, and that interface is an M3 deliverable.
- Publishing private derived values without rights is prohibited even when the
  values are "just colours".

## Assets, providers and user content

- TMDB branding (`tmdb-logo.svg` in the archive) remains TMDB branding, governed by
  TMDB's logo/attribution rules; it is not Screenroom artwork. If TMDB data is used,
  the required attribution sentence stays visible.
- Provider APIs (TMDB, OpenSubtitles) grant API use, not rights to redistribute
  other people's images or subtitle works. Indefinite offline retention of provider
  artwork is unresolved and is treated as a release blocker for automatic remote
  caching; user-permitted local image bytes with provenance are the lawful path.
- User videos, databases, posters and screenshots never become project assets
  because the app can open them. Test media must be generated or explicitly
  rights-cleared; the libmpv smoke test generates its own clip with FFmpeg.

## Release checklist (per package)

1. Dependency notices regenerated from the resolved lockfile of that build.
2. Native binaries pinned by hash with a recorded build configuration.
3. LGPL/GPL posture decided and documented; MPL files unchanged or published.
4. Private DS material confirmed absent from the artifact.
5. No credentials, certificates, DUIDs, personal paths or user media in the bundle.
6. Attributions visible where the provider's terms require them.

Review this document on every SDK, dependency, asset or native-build change.
