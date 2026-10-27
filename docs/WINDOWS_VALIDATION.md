# Native Windows validation — the exact handoff

**Status: nothing in this file has been run.** Windows is a **BLOCKED** gate. This
document exists so the first session that has the machine does not have to
rediscover what to run, what to expect, and which of the current claims are still
unproven. Every command here is the same command the macOS work used, and every
"expected" line is what the code is asserted to do — not a measurement.

Do not mark any row below as passed until the command has been run on real Windows
hardware and its output has been pasted into `IMPLEMENTATION_STATUS.md`.

## Why Windows is blocked from the Mac that develops this

`cargo check --target x86_64-pc-windows-msvc` cannot compile at all from macOS:
`libsqlite3-sys` builds bundled SQLite from C, and that needs the MSVC SDK's
headers (`fatal error: 'stdlib.h' file not found`). This is a toolchain boundary,
not a code failure, and its consequence is the important part: **every
`cfg!(windows)` branch in the tree is unbuilt as well as unrun** — the libmpv
candidate names, the D3D11 bridge choice, the Windows path rules and the
`BundleLayout::WindowsPortable` folder logic.

Three Windows defects were found and fixed on the Mac by reasoning about the rules
rather than by running them (commits `0f0a3b9`, `6eff910`, plus the layout work in
`caeea47`). Each fix encodes a documented Windows behaviour and each is asserted
from the Mac with Windows-shaped inputs. **What is still unproven is that Windows
accepts them.** The falsification of each is named in §7.

## 1. Prerequisites

| Need | Why | Note |
| --- | --- | --- |
| Windows 10/11 x64 | the gate | any supported build |
| Visual Studio 2022 **Build Tools** with **Desktop development with C++** (MSVC v143 + Windows 11 SDK) | `libsqlite3-sys` compiles bundled SQLite from C | the C++ workload, not just the Rust toolchain, is the requirement that is easy to miss |
| Rust stable, 1.85 or newer | the workspace is `edition = "2024"` | `rustup default stable` |
| Git | to clone the repo | |
| A self-contained libmpv build | the player | shinchiro's `mpv-dev-x86_64-*.7z` gives one `libmpv-2.dll`; a build that depends on other DLLs needs those beside it too, because Screenroom loads the one library and does not resolve anything else for you |
| FFmpeg and FFprobe on `PATH` | media probing, and the benchmark examples | optional for the acceptance walk; required by `library_bench`/`render_bench` if you run them |

No paid service, no account, no network access at runtime, and no remote access to
any machine is required or requested for any of this.

## 2. Build and test

```powershell
git clone https://github.com/beyourahi/screenroom; cd screenroom
cargo --version                       # 1.85+
cargo build --workspace               # expected: compiles, no MSVC header error
cargo test --workspace                # expected: 189 passed, 0 failed
cargo clippy --workspace --all-targets  # expected: no errors
cargo fmt --all --check               # expected: clean
```

Read the count carefully. `189 passed` is the macOS count, and the same number is
expected here, but **the same number does not mean the same coverage**: 

* two scanner tests have `#[cfg(unix)]` bodies and assert nothing on Windows;
* the twelve player worker tests and the two libmpv smoke tests print
  `skipped: …` and pass vacuously when no libmpv is found, so on a machine without
  the DLL they prove nothing about playback. Put the DLL in place (§3) before
  treating them as evidence.

Two `cfg!(windows)`-gated tests are the ones that have never executed anywhere: the
libmpv candidate names and the D3D11 bridge choice. **Their first run is here.** If
either fails, the fix belongs in this session, not in a bug report — the Mac cannot
see it.

## 3. Does the installed copy find its own libmpv?

Put `libmpv-2.dll` beside `Screenroom.exe` — the folder holding the executable, as a
portable install or an installer's program folder both do — then:

```powershell
cargo run -p screenroom-player --example capability -- --explain
```

Expected, and this is a **security** check rather than a preference:

* every executable-adjacent path is listed **before** any bare platform name,
  because Windows' own search consults the *working directory*;
* `platform search consults the working directory: true`;
* the order is `mpv-2.dll` (beside the exe) first among the names.

Then confirm the library actually loads and creates a core:

```powershell
cargo run -p screenroom-player --example capability
# expected: available: true, api version reported, bridge: Direct3D11
cargo run -p screenroom-player --example capability -- C:\path\to\libmpv-2.dll
# expected: same, naming that exact file — exit code 1 if it cannot be used
```

`bridge: Direct3D11` is the *starting choice*, not a proven path. See §6.

## 4. The library, the paths and removable storage

These are the checks for the three Windows defects fixed on the Mac.

```powershell
cargo run -p screenroom-desktop
```

| Check | Do | Expected |
| --- | --- | --- |
| Grant a folder on a fixed drive | Settings → add `D:\Media` and let it scan | rows appear; **no** "The source was replaced by a symbolic link" |
| A source survives a restart | close and reopen the app | the same folder is still listed and available; no rescan needed |
| Grant the same folder twice, spelled differently | add `D:\Media` then `d:/media/` | the second is refused as a duplicate — one folder, not two |
| Unplug USB, then rescan | grant `E:\Media` from a USB drive, unplug it, press Rescan | the source reads **"Drive not attached — reconnect it, then Rescan"**, every row stays, progress and lists stay, and the scan reports itself incomplete |
| Replug it | plug the drive back in, press Rescan | rows become available again; nothing was re-added as a duplicate |
| Delete a folder while its drive is attached | delete `D:\Media\Old` on a mounted drive, rescan | it reads **"Folder is gone — relink it to its new location"**, and Settings → *Relink and rescan* accepts a new location |
| Drive letter changes | if the USB drive returns as `F:` instead of `E:`, relink to `F:\Media` | files come back; history, favourites and resume positions are intact |

The last row is the removable-storage limit that is **by design**: Windows mints a
new drive letter for the same volume, and a letter is part of the spelling. There is
no volume-serial identity check yet, so the recovery is an explicit relink. Record it
as a limitation, not a defect, unless it fails to recover.

## 5. Display, input and the rest of the parity list

| Area | Check | Expected |
| --- | --- | --- |
| Ultrawide 3440×1440 | open the app on the ultrawide, full screen | more cards per row than on a 1280-wide window, each card roughly 300–500px, not four ~860px billboards |
| High DPI | set Windows scaling to 150% (and 200%), restart | text and controls stay crisp and proportionate; no clipping, no half-pixel hairlines |
| Narrow window | drag the window to its minimum | at least two columns; no crash, no zero-width card |
| Keyboard | arrows move one card (left/right) and one **row** (up/down); Enter opens; Escape goes back | focus lands on the card you expect after a resize too — the row step follows the drawn column count |
| Mouse | click a card, the transport buttons, a slider | same actions as the keyboard; sliders apply and persist |
| Playback | open a 1080p H.264 MP4 | plays with correct aspect; seek forward and back; pause/resume; close and reopen restores the position |
| Tracks and subtitles | open a multi-audio/multi-subtitle MKV | track selection switches; subtitle size and position sliders change the picture; the values survive a restart |
| Profiles, lists, settings | add a second profile, favourite something, hide a resume, change a setting | each profile keeps its own progress, lists, Continue Watching and hidden-resume |
| Backup and restore | backup to a chosen folder, then restore that file | the library comes back with the same works, progress and lists |
| Failure recovery | grant a folder you have no read permission on, then rescan | it reads "Access was withdrawn — grant access to the folder again", and nothing is deleted |

## 6. What is deliberately not claimed on Windows

1. **The D3D11 presentation path does not exist yet.** libmpv can render to D3D11;
   Screenroom presents frames through wgpu with a software bridge. The Windows
   equivalent of the macOS zero-copy adapter is designed and unbuilt, so on Windows
   you are looking at the CPU bridge — the same one measured at 1080p on macOS, with
   unknown 4K headroom. `RenderBridge::Direct3D11` names the *intended* bridge, not a
   working one.
2. **4K, HDR and hardware decode are unmeasured everywhere.** The macOS benchmark
   that reads as "4K" actually measures 1080p (`MAX_DIMENSION = 1920`).
3. **Nobody has heard audio.** The audio clock and `avsync` are measured; listening
   is not.
4. **No Windows packaging or signing.** `scripts/package-macos.sh` and
   `scripts/verify-bundle.sh` are macOS-only. There is no Windows installer, no
   `.exe` packaging step and no Windows bundle verification; running from
   `cargo run` is the only supported form today. Distribution of a libmpv/FFmpeg
   build is a licensing decision recorded in `LICENSING.md`, not implied by the code.

## 7. Named falsification — what would prove each fix wrong

Stated so a failure is diagnosable rather than mysterious.

| Fix | Would be wrong if | Where to look |
| --- | --- | --- |
| Executable-adjacent DLL search before the platform search (`0f0a3b9`) | `--explain` lists a bare name first, or a stray `libmpv-2.dll` in the *working directory* is loaded when the install's own copy is present | `crates/screenroom-player/src/lib.rs`, `library_search_order` |
| Verbatim-prefix path identity (`6eff910`) | granting a healthy folder reports "replaced by a symbolic link", or the same folder added twice is accepted | `crates/screenroom-core/src/paths.rs`, `location_key` |
| Grid sized to the window (`caeea47`) | an ultrawide still shows four wide cards, or arrow-Down lands on the wrong card after a resize | `apps/desktop/src/app.rs`, `grid_columns` |
| Folder-gone vs drive-unplugged (`3db6032`) | deleting a folder on a mounted drive says "Drive not attached", or an unplugged drive says "Folder is gone" | `crates/screenroom-core/src/scanner.rs`, `unavailable_root` |

A failure in any row is a real Windows bug with a known location. Hand the failing
command and its output back to the Mac session, which can fix the rule and assert it
without the machine.

## 8. What to bring back

For each row above, the command, its output, and the observed on-screen behaviour.
Then update, in the same commit:

* `docs/IMPLEMENTATION_STATUS.md` — the test count for Windows, and the blocked-gate
  table;
* `docs/FEASIBILITY_GATES.md` — G3/G4 for Windows;
* `docs/CAPABILITY_MATRIX.md` — the Windows desktop row.

Nothing in this document becomes "verified" from a build succeeding. A build is not
a played frame, and a played frame is not a measurement.
