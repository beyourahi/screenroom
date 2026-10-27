> **SCOPE AND STACK NOTICE (2026-10-26):** [`ADR-002_RUST_FIRST_STACK.md`](ADR-002_RUST_FIRST_STACK.md) as amended by [`ADR-003_DESKTOP_AND_TV_SCOPE.md`](ADR-003_DESKTOP_AND_TV_SCOPE.md) is the authoritative architecture: desktop/laptop (Windows, macOS, Linux) and smart TVs (Samsung Tizen, LG webOS, Android TV, other compatible TVs) only, with equal priority; shared Rust core, iced + wgpu + libmpv desktop, LightningJS 3 + Blits + TypeScript TV with platform-native playback, SQLite where supported, FFmpeg/FFprobe, MIT first-party code. iOS/Android phone and tablet apps and the Flutter fallback are out of scope. This document's product requirements, entities and evidence rules still apply to desktop and TV; any mobile, Flutter-only or Svelte/Tauri statement inside it is history, not a requirement. No physical-device gate has passed -- see `IMPLEMENTATION_STATUS.md` for what is actually verified.

# Test strategy, evidence matrix and exit criteria

**Mandatory correction (2026-10-25):** test actual installed packages with browsers closed and no network/server; measure resource budgets rather than assuming a thin frontend equals a fast application. Adopt the private Dropout Design System by approved vendoring, enforce component/tokens parity, generated CSS M76 support, visual dark-theme parity, borderless focus and couch-distance D-pad usability. See `NATIVE_APP_STRATEGY.md`, `DROPOUT_DESIGN_INTEGRATION.md`.

**Current state (2026-10-28):** the desktop application and its shared core are
built and tested; no physical-device test has been run. The archived v0.1 reported
6 unit tests/browser Chromium smoke/mocked TV methods and did **not** run on actual
Windows/Samsung hardware. Do not count source review as passed tests.

**PENDING, needs a visible window and an owner-approved session** (recorded here so
they are consolidated rather than quietly skipped):

- the keyboard journeys in §4 (`W-01`/`W-02` equivalents on macOS);
- the disc-removal cases in §4 (`R-01`);
- the **on-screen** rendering and input budget at 10,000 items — the store half is
  measured in `IMPLEMENTATION_STATUS.md` (M1) and the offscreen half by
  `render_bench`, but iced's own frame cost and event-to-redraw latency cannot be
  measured without a window;
- re-running `scripts/verify-quit.sh`, whose evidence predates the background-first
  policy;
- the case-sensitive-volume half of the path-semantics tests, which needs a
  case-sensitive APFS volume rather than this host's folding one.

**Windows cannot be compiled from this Mac at all.** `cargo check --target
x86_64-pc-windows-msvc` fails inside `libsqlite3-sys`, whose bundled SQLite needs
the MSVC SDK headers (`fatal error: 'stdlib.h' file not found`). So every
`cfg!(windows)` branch — the libmpv candidate names, the D3D11 bridge choice, the
Windows path rules — is unbuilt as well as unrun. That is a toolchain gap rather
than a code failure, and it is why Windows stays a blocked gate rather than a work
item that is merely untested.

Nothing in this document has been run against Windows or a TV.

## Testing without taking over the desktop

**Policy (2026-10-28): background-first.** Verification runs on the owner's own
MacBook while they are using it, so no check may bring Screenroom to the front,
switch Spaces, move an active window, or take the keyboard and mouse. Foreground
UI automation is opt-in via `SCREENROOM_ALLOW_FOREGROUND=1` and refuses without it;
`scripts/foreground-guard.sh` is the gate. The full rule is in `AGENTS.md`.

| Tier | Opens a window | Takes focus | What it can prove |
|---|---|---|---|
| `cargo test --workspace` | no | no | Domain, storage, scanner, player worker against real libmpv, paging at scale, checkpoint policy |
| `cargo run -p screenroom-player --example capability` | no | no | Which libmpv this host resolved |
| `crates/screenroom-player/examples/audio_clock.rs` | no | no | Audio clock against real decoding |
| `crates/screenroom-player/examples/render_bench.rs` | no | no | Decode → render → frame delivery rate, time to first frame and command latency, entirely offscreen |
| `scripts/check-quit-routing.sh` | hidden | **no** | That the menu bar's Quit was re-pointed at the window's close action |
| `scripts/verify-quit.sh` | yes | **yes** — opt-in only | That a real dismissal stores the final position and closes the session |
| `scripts/drive-app.sh`, `capture-window.sh` raise | yes | **yes** — opt-in only | Screenshots and keyboard journeys |

`scripts/check-quit-routing.sh` exists because the menu bar only exists inside a
running application: it launches the real binary with `SCREENROOM_QUIT_CHECK` set,
which makes the process hide its window, examine the menu bar, write the outcome to
a file and exit. Measured with the frontmost application sampled every 50 ms for the
duration of the run, it stays whatever it was — on 2026-10-28, Google Chrome
throughout.

What this tier **cannot** prove: that quitting stores a position. That needs a real
window and a real ⌘Q, so it belongs to `verify-quit.sh` and to a permissioned
session. Recording it as passed on the strength of the hidden check would be
claiming a test that did not happen.

`render_bench` has the same limit in a different place: it measures the real decode
and render path with no window, but **not** an on-screen frame rate. iced, the wgpu
upload and the compositor add cost it cannot see, so its numbers are a ceiling on
what the display can reach, not a substitute for measuring the display.

What runs today (`cargo test --workspace`, **168 tests, 0 failures** on macOS ARM64):

| Suite | Proves |
| --- | --- |
| `screenroom-core` unit | filename interpretation, fingerprints |
| `core/tests/library.rs` | sources, scanning, ingestion, paging, playback invariants, lists, per-viewer isolation, path case semantics on both branches |
| `core/tests/storage.rs` | typed preferences including subtitle size/position (refused outside range on write, clamped on read), backup round-trip, restore validation and its refusals |
| `core/tests/scanner.rs` | bounded scan, cancellation, symlinks, settle window, an unplugged drive, a root replaced by a symlink |
| `core/tests/recovery.rs` | crash-state normalisation, damaged and future databases, progress across recovery |
| `core/tests/scale.rs` | 10,000-item paging, search and count inside a budget |
| `screenroom-player` unit | state machine, options, property mapping |
| `player/tests/worker.rs` | real libmpv decode, frame rate, audio clock and `avsync`, track switching, **subtitle style reaching libmpv and surviving a file change**, failure states, long session |
| `player/tests/libmpv_smoke.rs` | client API on this host |
| `screenroom-desktop` unit | navigation, filters, autoplay decision, resume copy, close-path state, slider-percent rounding, agreement between the offered subtitle range and what the player accepts, and that the menu-bar routing is found by action rather than by title |
| `desktop/tests/playback.rs` | checkpoint policy against a real SQLite library and a real clip |
| `scripts/check-quit-routing.sh` | that the running application re-pointed the menu bar's Quit (hidden window, no focus taken) |

Everything below about Windows/TV tiers, browser tiers and the Svelte UI is
historical planning; the platforms it names were narrowed by ADR-004.

## 1. Test tiers and ownership

| Tier | Tests | Outcome proves |
|---|---|---|
| Unit (`U`) | naming/season order/manual override/progress/state/backup validation/serializer | Pure shared-domain invariants on synthetic data. |
| Contract (`C`) | typed playback/storage/source ports, time units, capability/error payloads, native IPC validation | Adapter obeys contract; **not** real device capability. |
| Integration (`I`) | SQLite file/transaction/scanner, FFprobe/FFmpeg worker, file/artwork store, mock provider quota/corrupt response | Real process/storage behavior on a tested host; not Samsung unless run there. |
| Browser (`B`) | Svelte UI, search/scroll, keyboard, focus, actual browser-playable test videos, screenshots/axe, M76 compatibility build | Shared UI works in browser/tested engine; not native codec/TV USB proof. |
| Windows on-device (`W`) | signed/test-installed shell, native libmpv surface, GPU/codec/subtitles/aspect/resume, actual ultrawide | **Windows hardware-specific** behavior. |
| Samsung on-device (`T`) | signed Tizen install, USB read, AVPlay+native audio, M76 UI, remote focus, data persistence/power-cycle | **This physical TV's** behavior. Emulator/mocks are separate tier. |
| Recovery/security (`R`) | crash, remount, failed migration, invalid backup, permission loss, resource pressure, untrusted path/provider | Relevant risk property under target environment. |

Never aggregate tiers into a universal “green” status. Every release report must separate **implemented**, **unit verified**, **browser verified**, **Windows verified**, **Samsung verified**, **blocked**, and **deferred/accepted** per feature and stream.

## 2. Media fixture matrix

All test media must be **generated or rights-cleared**, tiny except bounded stress samples; media spec manifest includes SHA-256, container, video codec/profile/bit depth, fps, dimensions, audio codec/channels, subtitle format/language, origin/license and expected platform support. Never commit personal copyrighted movie files or credentials.

| Fixture | Primary assertion | Devices |
|---|---|---|
| MP4 H.264 1080p + AAC stereo | baseline seek/resume, audio, correct aspect | Windows/TV |
| MP4 HEVC Main10 2160p + AAC | 10-bit/4K, real HW decoder and performance, truthful fallback | Windows/TV |
| MKV HEVC + EAC3/AC3 | container/audio profiles/track select | Windows/TV if documented |
| MKV DTS-only track | TV unsupported audio is clearly communicated; desktop replacement/convert option | Windows/TV |
| MKV multi-audio/multi-subtitles | track switching/profile/language and state restoration | Windows/TV per capabilities |
| External SRT/VTT/SAMI plus Bengali Unicode | actual supported subtitle formats, encoding, timing offset | Windows/TV, supported distinctions |
| Embedded ASS/PGS/image subtitles | no fake universal support; Windows render vs TV fallback | Windows/TV |
| Very short and 2-hour-long video | seek end, credits/watched heuristic, stable time units | Windows/TV |
| Bad index, partial download, corrupted/wrong extension | actionable errors/retry/no crash/data loss | Windows/TV |
| Alternate cuts and 2 episodes/file | edition/gap ordering/progress identity | Shared domain + both |
| Files with whitespace/Unicode/long paths | secure real read and no command interpolation | Windows/TV file systems |
| Conditional AV1/HDR/HFR | strictly capability-reported based on actual device | Windows/TV only if relevant |

**Observed compatibility ≠ file extension compatibility.** Record video codec level/profile, audio and subtitle individually.

## 3. Requirements-to-test coverage

| Feature IDs | Required checks |
|---|---|
| F01–F02 | authorized add/remove/relink, no source mutation, hotplug, cancel/resume, 10k files, unreadable entries partial success, symlink/duplicate loops and interrupted downloads. |
| F03–F04 | unknown titles, ambiguous remakes, anime/specials, combined episode mapping, manual overrides preserved after enrichment/rescan, alt edition. |
| F05–F06 | online every-connected-title queue, quota backoff/stop/restart, metadata/image byte persistence **offline after relaunch**, provenance/rights/expired cache behavior. |
| F07 | genuine Windows libmpv and TV AVPlay decoding; aspect, pause/seek/fullscreen, tracks, hardware support, TV USB local URI; headset/speaker state where relevant. |
| F08–F10 | distinct profile progress, stale session event/seek race, periodic checkpoint/crash/reboot, Continue Watching order and marked watched behavior, end/gaps/cross-season rules. |
| F11–F12 | all 10k fixtures reachable via search/filter/sort/virtualized scroll, scroll restoration, watchlist survives relink/rename/match correction. |
| F13–F15 | audio/embedded/adjacent subtitle and timing variants, malformed subtitle, quota/provider outage, downloaded subtitle license and reuse, edition-specific manual and provider skip markers. |
| F16–F19 | cold-start/no-genre recommendations, missing-file exclusion, preview one-decoder/reduced-motion, real PiP controls, receiver accessibility + opt-in LAN session stop. |
| F20–F22 | versioned import/export, corrupt/malicious backup, migration rollback, source/codec/permission/provider errors and safe credential exclusion. |
| F23–F24 | source accuracy/provenance/protocol and `NOT_CHECKED` state; exact endpoint + ISP reach only if legitimately tested, no illegal/unauthorized automation. |
| F25 | FFprobe choice of remux/audio-transcode/full conversion; separate output; space estimate, cancel/interruption cleanup, validation and edition-time remap. |

## 4. Hardware/lifecycle acceptance

**W-01:** collect Windows OS/GPU/CPU/monitor and native video output settings (without personally sensitive IDs). Open baseline video with desktop shell, native playback, correct aspect on ultrawide and custom controls; record actual first frame/perf.

**W-02:** repeated seek/track/fullscreen/resize/suspend/reopen and switch profile; no stale progress updates. Exercise all test formats and list actual successes/failures, including hardware decode path.

**T-01:** developer mode, signed on-device installation on UA50AU8000RXER, list USB storages and resolve selected file to absolute URI; AVPlay H.264/AAC video+audio and remote seek/Back. No PC server involved.

**T-02:** prove `wgt-private` data + poster bytes survive normal app restart, unplug network and TV power cycle; distinguish in-place update vs Tizen Studio reinstall; restore backup when uninstall erases data.

**T-03:** USB removal/reconnect, D-pad 100+ items, modal/picker focus return, media keys, 4K/10-bit/unsupported DTS, subtitle options and suspend/exit repeated. Log actual model firmware, storage FS and tested decoder matrix.

**O-01:** no network: open app, browse previously imported library/artwork, play accessible file, checkpoint and resume with profiles intact. Absence of cloud/backend is tested with network explicitly disabled.

**R-01:** wrong metadata match, moved file, changed mount, duplicate root, corrupt library import, insufficient cache space, provider 429 and crash while checkpointing. No original modifications or unexpected state reset.

## 5. Proposed performance criteria (not baseline observations)

- Define a device-specific performance budget after M0: cold app-to-interactive, warm restart, first relevant scan item, 95th percentile input feedback, 95th percentile rail scroll/focus, first video frame, sustained 30-minute/2-hour viewing with CPU/GPU/RAM and subtitle/audio stability.
- A **proposed engineering target**, to revise after measuring hardware: focus/key feedback generally under ~100 ms, avoid visible multi-frame jank while browsing, no heavy indexing/conversion during active playback, no whole-library rerender per progress tick; do not promote to a claimed tested guarantee.
- Repeat both with a tiny catalog and synthetic 10,000-record catalog; check memory exhaustion/eviction after many pages/preview attempts. Extreme stress may be deferred only if disclosed and reasoned.
- TV CSS/JS tested on **Chromium M76** APIs and actual 2021 television, not merely compiled to `es2019` or viewed in desktop Chromium.

## 6. Accessibility, privacy and software quality

Keyboard/remote-only across key journeys; visible focus, contrast, reduced motion, screen reader names where platform supports, Unicode subtitles, preservation of focus after metadata updates. Security review: media-root auth, path traversal/symlinks, provider redirect/URL allowlist, subtitle/import XSS, untrusted NFO parsing, native IPC bad values, FFmpeg command argument safety, API credential redaction; no accidental Git check-in of videos/logs/private device files.

Run local checks (strict $0): formatting, lint, Svelte/TS diagnostics, Rust format/clippy/tests, unit+contract tests, browser Playwright/axe as appropriate, integration jobs and device-specific checks. No mandatory paid CI runner. Keep commands in root package scripts once introduced.

## 7. Evidence receipts and release exit

Record each test outcome with `test ID`, device+OS/firmware, git commit, date, exact fixture spec, steps, assertion, observed result, screenshot/log path, and status `PASS | FAIL | BLOCKED | NOT_APPLICABLE`. Store sensitive raw receipts in ignored `.artifacts/`, keep only redacted summaries tracked. The compiler passing is **not** the same as G1/G2 or on-device proof.

**Personal release exit:** real Windows and Samsung playback verified for declared formats + appropriate unsupported-media fallbacks, offline restart/binary artwork rights and storage demonstrated, persistent profiles/progress, source-not-found states, backup/migration, remote/keyboard access, and every F01–F25 row reconciled (implemented or expressly accepted limitation). Recheck any legal/terms blockers before publishing or distributing; no commercial release authorization here.
