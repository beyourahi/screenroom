# Architecture and monorepo specification

> **2026-10-27:** this document describes the Rust-first architecture selected by
> [`ADR-002_RUST_FIRST_STACK.md`](ADR-002_RUST_FIRST_STACK.md), scoped to
> desktop + TV by [`ADR-003_DESKTOP_AND_TV_SCOPE.md`](ADR-003_DESKTOP_AND_TV_SCOPE.md)
> and then to the owner's three Personal Edition devices by
> [`ADR-004_PERSONAL_EDITION_SCOPE.md`](ADR-004_PERSONAL_EDITION_SCOPE.md).
> Statements in older documents that assert a Flutter-only or Svelte/Tauri stack,
> or that list Linux, webOS or Android TV as targets, are superseded and kept as
> history.

**Status:** the shared core, the libmpv boundary and the desktop application are
implemented and verified on the macOS host, including real frame presentation
(see the playback section). No physical-device gate for Windows or the Samsung TV
has been satisfied. See [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md) for
the live ledger.

## Architecture goals

1. **One repository** holds every first-party crate, app, TV package, test,
   script and decision document. Dependencies come from normal registries/SDKs;
   no submodules or sibling repos.
2. **One shared core** in Rust owns identity, the SQLite library, scanning,
   progress rules and lists. The desktop app links it natively; TV clients mirror
   its contract.
3. **Native playback per device.** libmpv on desktop; AVPlay on Tizen, the webOS
   media pipeline on webOS, Media3 on Android TV. No shared "player abstraction" is allowed to
   pretend these behave identically.
4. **Offline-first by construction.** UI, fonts, assets and user data are local.
   Network providers (metadata, subtitles) are optional enrichment behind an
   explicit consent boundary and never block the library.
5. **Durable, honest state.** Missing drives, revoked permissions, corrupt
   databases and unsupported codecs produce specific, non-destructive states.
6. **Bounded workload.** Scanning, enrichment and conversion are bounded,
   cancellable and resumable; watching has resource priority.

## Layout

```text
screenroom/
  Cargo.toml                     # Rust workspace (apps/desktop + crates/*)
  crates/
    screenroom-core/             # domain, SQLite, scanner, filename, fingerprint
      src/{domain,error,filename,fingerprint,scanner,schema}.rs
      src/library/{mod,sources,catalog,playback,lists,settings,backup}.rs
      tests/{library,storage,scanner,recovery,scale}.rs
    screenroom-player/           # libmpv runtime boundary + player state machine
  apps/
    desktop/                     # iced + wgpu client, video_surface.rs bridge
    tv/                          # LightningJS 3 + Blits client (Samsung Tizen only)
      src/core/                  # the core's contract and rules, ported to TypeScript
      src/platform/              # the Tizen boundary: AVPlay, remote keys, storage, USB
      src/design/                # semantic tokens at TV scale, card and grid model
      src/screens/               # Home · Browse · Detail · Settings · Player
      src/app/                   # runtime assembly, player runtime, route props
      test/                      # 82 off-device tests (node --test, no TV needed)
      config.xml                 # the widget manifest and its privileges
      scripts/prepare-tizen-dist.mjs  # bundle -> installable widget directory
  scripts/                       # evidence helpers + the macOS .app packager
  docs/  reference/  .worktrees/ #  .worktrees is Git-ignored, archive branches
```

### How `apps/tv` differs, and why it is not a port

The television does not run the Rust core, so `apps/tv` is a second implementation of
the same *contract* rather than a second build of the same code. It mirrors
`domain.rs`'s entities and its two travelling rules — identity is an
application-generated id rather than a path, and an unavailable source never deletes
anything — in TypeScript, and it re-implements the rules that decide behaviour as pure
functions with their own tests (`core/focus.ts`, `core/progress.ts`, `core/store.ts`,
`core/locator.ts`).

The boundaries are ports, so the platform is replaceable and testable: `StoragePort`
takes bytes in and bytes out, and the durability rules live above it; the playback port
has an AVPlay implementation and a browser-preview one that is named as such in the UI,
because a preview that reads like a device result is worse than no preview.

The build is deliberately unlike the desktop's: one bundle, relative paths, targeted at
`es2019` for the 2021 engine, because a widget has no server and no origin. Everything
the Mac can verify about it is verified; nothing about the engine, the renderer or
AVPlay is claimed. See [`TIZEN_DEVICE_HANDOFF.md`](TIZEN_DEVICE_HANDOFF.md).

The Flutter candidate lives on `archive/full-app-2026-10-25`,
`archive/source-check-2026-10-25` and `archive/cache-controls-2026-10-25` and in
`.worktrees/`. It is preserved, not built. `reference/` holds the historical
Svelte/Tauri prototype.

## Dependency direction

```text
        ┌──────────────────────────── screenroom-core (Rust) ───────────────────────────┐
        │ domain · SQLite schema/migrations · scanner · filename · fingerprints ·      │
        │ progress invariants · lists · availability                                   │
        └───────────▲──────────────────────────────────────────▲────────────────────────┘
                    │                                          │
        apps/desktop (iced+wgpu)                     TV contract harness
                    │                                          │
        screenroom-player (libmpv)                       Tizen AVPlay
```

`screenroom-core` never depends on a UI, a player or a network client. That is
enforced by its dependency list, not by convention: it has no UI or HTTP crate.

### Core contracts callers may rely on

- **Identity:** UUID strings generated by the app. Never a path, filename or
  provider ID. A provider ID is an attribute in `external_identities`.
- **Time:** integer milliseconds everywhere. Native units are converted at the
  adapter boundary.
- **Progress:** accepted only from the authoritative
  `(session, generation, sequence, seek_generation)` tuple. Stale native callbacks
  are rejected, never "clamped".
- **Availability:** a missing source or file flips `available`/`state`; nothing is
  deleted. Works, editions, progress, lists and markers survive a disconnected
  drive, a rename or a permission revocation.
- **Paging:** library queries are paged (`1..=500` per page) with a stable sort so
  a 10k-item library never loads at once.
- **Errors:** stable `code()` strings (`duplicate_source`, `scan_running`,
  `stale_session`, `future_database`, …) plus a human message that is not a
  stable contract.
- **Preferences:** typed over the existing `settings` key/value table, so no
  physical layout changed. A value that reaches libmpv as an argument (a language
  tag) is validated first, and a rejected save writes nothing at all.
- **Backup:** a validated SQLite copy, not a JSON export. It is written by
  `VACUUM INTO` through a same-directory staging file and a rename, so an
  interrupted backup leaves no partial file. Restore validates format, schema range
  and integrity, copies the current library aside, then swaps in one transaction —
  an interrupted restore leaves the previous library intact.
- **Track choice:** the core stores a preference; the actual track list comes from
  libmpv and the selection is read back from the player snapshot. The UI never keeps
  a second copy of the truth.

## SQLite and migrations

One physical schema (`crates/screenroom-core/src/schema.rs`) and one
`user_version` (`SCHEMA_VERSION = 2`) for every platform. The layout deliberately
matches the schema the Flutter candidate wrote, so an existing library on a device
opens without a destructive migration.

- Opening applies `PRAGMA foreign_keys = ON` and `busy_timeout`.
- A schema version of `0` creates and stamps; `2` opens; `1` fails closed with
  `migration_unavailable`; anything newer fails with `future_database` and leaves
  the file untouched.
- `PRAGMA quick_check` must return `ok`, otherwise `corrupt_database` and the file
  is left for recovery/restore.
- Opening closes dangling sessions and re-queues `running` jobs: a crashed process
  is never authoritative.
- TV clients that cannot use SQLite must implement the same entities with the same
  invariants and pass the shared contract tests; that adapter is still unbuilt.

## Scanning

`LibraryScanner` walks one authorized root, breadth-first, with hard caps:
250,000 entries, depth 64, 15-second settle window, 1,000 issues retained plus an
explicit `additional_errors_omitted` marker.

- Symbolic links are never followed (loops and root escapes are impossible).
- Hidden directories are skipped; non-media extensions are ignored.
- A file with a young mtime, zero size, a download sidecar (`.part`,
  `.crdownload`, …) or a changed size/mtime during hashing is reported as
  `still_downloading` and not imported.
- One unreadable folder produces a per-entry issue; the walk continues.
- Only a complete scan (no bound hit, no issue) withdraws availability, so a
  cancelled or bounded scan can never make a library look empty.
- Two concurrent scans of the same source are rejected with `scan_running`.

Ingestion is incremental: identical rescan keeps the row; replaced bytes at the
same path get a **new edition** so the old cut keeps its own progress; a small file
that reappears with the same fingerprint under a new name keeps its identity when
the old locator is gone.

## Playback

`screenroom-player` loads libmpv at runtime (`dlopen`, no link-time dependency),
so a device without libmpv reports `available: false` with a reason instead of
crashing at start-up. It wraps the client API only: core creation, options,
command arrays, property reads and event waits.

**The rendering bridge.** libmpv's render API supports OpenGL, Direct3D 11 and
software only; iced draws through wgpu/Metal/Vulkan/DX12, and there is no
zero-copy handshake. `apps/desktop/src/video_surface.rs` implements the portable
**software bridge**: mpv's RGBA frame is uploaded into a wgpu texture in
`Primitive::prepare` and drawn letterboxed by a WGSL shader. It presents real
frames on the macOS host (verified 2026-10-27); the OpenGL (macOS) and D3D11
(Windows/Arc) zero-copy paths are planned behind the same `Frame`/`VideoSurface`
seam.

The frame path, in the order a frame travels:

1. libmpv renders into a 64-byte-aligned, stride-padded buffer (`render.rs`).
2. `SoftwareRenderer::pack` **tightens each row with one `copy_from_slice`**. It
   previously rewrote all four bytes per pixel; that cost ~40 ms for a single
   1080p frame in an unoptimized build and capped presentation at ~11 fps. Row
   copies took the measured rate to 30.0 fps on the same media (see the gates).
3. `Frame { rgba: Arc<Vec<u8>>, generation }` is published to the shared
   snapshot. Red, green and blue are the picture; **the alpha byte is undefined**
   (libmpv documents `rgb0` that way), so the presenter forces opaque output and
   no host code inspects it.
4. The desktop primitive uploads the frame only when `generation` changes, then
   draws. `PLAYER_TICK` (16 ms) is shorter than a frame at 60 fps, so the window
   is fed at the media's own rate.

Because the render context and the wgpu device must not be mixed across threads,
the player owns its own thread and hands frames over as `Arc<Vec<u8>>` with a
monotonic generation number; the UI never blocks on decode.

Player state machine: `Idle → Resolving → Preparing → Playing ↔ Paused →
Ended | Failed`, where `Ended` and `Failed` are terminal. `Ended` being terminal
is why the app's transport control *reopens* a finished file instead of clearing
the pause flag — clearing it would do nothing at all. Progress is only meaningful
in `Playing`, `Paused` or `Ended`.

## Platform adapters

| Target | Chain |
| --- | --- |
| MacBook Air M4 (macOS ARM64) | iced window + wgpu (Metal) + libmpv software bridge + SQLite — **running** |
| Windows desktop (x86_64, Intel Arc B580) | iced window + wgpu (DX12) + libmpv (D3D11 bridge target) + SQLite — same crate, not built on Windows |
| Samsung UA50AU8000RXER (Tizen 6.0) | Lightning 3 + Blits bundle (`.wgt`) + AVPlay + local-storage adapter — **built, tested and packaged; has never executed on a Tizen engine** |

A platform label is not a capability. Each capability is recorded per device in
[`CAPABILITY_MATRIX.md`](CAPABILITY_MATRIX.md) with the state of its evidence.

## Security boundaries

- Media roots are granted explicitly, canonicalized and checked for overlap. A
  replaced symlinked root is refused (`unsafe_path`).
- Relative locators are validated: absolute paths, `..` and NUL are rejected.
- FFmpeg/FFprobe and libmpv are invoked with fixed executables and argument
  arrays, never a shell string built from a filename.
- Imported metadata and subtitle text are data, never markup or code. NFO parsing
  is bounded and refuses DTDs/entities.
- Secrets are never persisted in the library, backups or logs; provider
  credentials are session-scoped until a platform secure store is implemented.
- No privileged web view, no arbitrary URL loading, no localhost server.

## Delivery

One Git repository, MIT for original first-party code, lockfiles committed. Native
binaries (libmpv/FFmpeg builds) are not committed; their license, hash, build
options and notices are a release-time obligation recorded in
[`LICENSING.md`](LICENSING.md). TV signing material, certificates and DUIDs never
enter Git. No store submission is automated.

**Desktop installation.** `scripts/package-macos.sh` assembles `dist/Screenroom.app`
with the release binary, an `Info.plist` and its own copy of libmpv in
`Contents/Frameworks`. The player searches the bundle after the system paths
(`packaged_candidates` in `crates/screenroom-player`), so an installed app keeps
playing when the Homebrew prefix changes or a development copy disappears. libmpv's
transitive dependencies still resolve by absolute Homebrew path and the bundle is
unsigned: both are deliberate for a personal install and are reported by the script
rather than papered over.

**Exit is a state transition.** The window's close request runs the same graceful
path as Back — final checkpoint, then `end_session`. ⌘Q terminates the process
below the event loop and cannot be intercepted, which is exactly why the next
launch's recovery normalisation (closing dangling sessions) exists.

## Revisit criteria

Measured failure, not preference, reopens a decision:

- If no bridge keeps desktop presentation at target resolution on the Windows
  machine, reconsider the desktop shell (not the shared core). On macOS the
  software bridge already sustains the media's full rate; the Windows question is
  whether Arc hardware decode and a D3D11 zero-copy path are needed for 4K/HDR.
- If Lightning 3 cannot hold its frame budget on the Tizen 6.0 engine, evaluate a
  lighter Blits-only build for that model.
- Any new platform family needs its own ADR. Retired targets (Linux, webOS,
  Android TV, iOS/Android) are listed in ADR-004 and are not revisited by
  preference.
