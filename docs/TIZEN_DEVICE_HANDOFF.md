# Samsung TV device handoff

**Target:** the owner's Samsung **UA50AU8000RXER** — a 2021 Crystal UHD running the
Tizen 6.0 web engine. This is the only TV in scope
([ADR-004](ADR-004_PERSONAL_EDITION_SCOPE.md)).

**State: gate G5 is BLOCKED on the device, and materially further along than it was.**
Everything that can be checked without the television has been checked and is listed
below, because when the device is finally attached there should be nothing left to
discover about the build — only about the TV.

The rule this document exists to follow: a packaged `.wgt`, a signed package, a Tizen
Studio install or a television emulator is **not** a TV test. Only the television is.

---

## 1. What is already verified (no TV needed)

```bash
cd apps/tv
pnpm install                 # Blits 2.15.0, TypeScript 5.9, Vite 7
pnpm typecheck               # app source and tests, separately
pnpm test                    # 82 tests, 0 failures
pnpm build                   # typecheck + Vite + the Tizen staging step
```

| Check | Result (2026-10-28) |
| --- | --- |
| `pnpm typecheck` | clean. Application source typechecks with **no ambient types at all** (`"types": []`), so a Node-only global cannot typecheck inside a component that has to run in a web widget; the tests are the only files that see Node's globals |
| `pnpm test` | **82 tests, 0 failures** — the focus machine, the progress guard, the store and its durability rules, the relative-locator validation, AVPlay's display-rect maths and track reading, the design tokens, and the card/grid geometry |
| `pnpm build` | one 428,957-byte `main.js` and a 1,391-byte `index.html`; no code splitting (a widget has no server to fetch a second chunk from) |
| `apps/tv/build/widget/` | the staged widget: `config.xml`, `icon.png`, `index.html`, `main.js`, and the Tizen project metadata. The staging step refuses to continue if the bundle is unlinked, missing, or referenced by an absolute path |
| Signed package | **built.** `apps/tv/build/ScreenroomTV-0.1.0.wgt`, 147,404 bytes, sha256 `ec40f2cebe29da19ec8911d2e3342263294aab6096f169b6744e0eca37ee05d0` |
| The packaged bundle | byte-identical to the tested one: `main.js` sha256 `1912be6e…ba98` inside the `.wgt` and in the staging directory |
| Signing profile | one profile exists and is **active**: `screenroom` (name read only; no certificate material was inspected) |

The package was produced through the `tizen-build-project` runner
(`tizen-sdk-skills`), never by hand-typed `tz` commands:

```bash
node ~/.codex/plugins/cache/tizen-platform/tizen-sdk-skills/*/lib/cli/project-manager-cli.js \
  build --project "$PWD/build/widget" --build-type Debug
```

Its envelope reported `status: success` and **no warnings** — in particular not the
"Signed with Tizen default developer certificates" warning, which is the signal that a
build fell back to the SDK's test certificates. Default-signed packages install on an
emulator only, so this is the difference between a package that can reach the TV and
one that cannot.

### What that does **not** mean

- It does **not** mean the TV will accept the install. The distributor certificate in
  the `screenroom` profile must list *this television's* DUID. The TV rejects anything
  else with `Invalid certificate chain`. That is a one-command check on the TV and it
  is step 2 below.
- It does **not** mean the bundle runs. Nothing in this project has ever executed on a
  Tizen engine: not the renderer, not WebGL, not a single frame.
- It does **not** mean the app plays video. No byte of media has passed through AVPlay.

---

## 2. First: is this TV authorised to install it?

Connect the TV (developer mode on, host PC IP entered in the TV's Apps settings) and:

```bash
sdb connect <tv-ip>:26101
sdb devices                      # the TV must appear, not just "List of devices attached"
```

If the profile's distributor certificate does not contain this TV's DUID, recreate the
profile with the Samsung certificate flow (`tizen-certificate-manager`) before
installing. Do not debug a rejected install as though it were a code problem: an
`Invalid certificate chain` failure is a certificate problem every time.

**Reinstall destroys local data.** A Tizen Studio reinstall is an uninstall followed by
an install, and `localStorage` for the widget goes with it — profiles, positions and
watch state included. An in-place package update over an installed app keeps them;
this has not been observed and is item 6 of the checklist for exactly that reason. Take
an external copy of anything worth keeping before the first reinstall.

---

## 3. Install and run

```bash
node ~/.codex/plugins/cache/tizen-platform/tizen-sdk-skills/*/lib/cli/install-app-cli.js \
  <path>/build/ScreenroomTV-0.1.0.wgt --run
```

(`tizen-install-app` owns this; nothing here should be typed as a raw `sdb install`.)

The app must come up as **Screenroom**, not a blank canvas and not the emulator's
fallback. If it is blank, the first thing to read is the widget's console — the app's
own startup probe reports what it found, and the Settings screen (the remote's **Info**
button) prints it again on screen: TV model, Tizen version, playback path, which remote
keys were accepted, and whether storage accepted a write.

---

## 4. The acceptance checklist

Each numbered item is a separate receipt for gate G5
([`FEASIBILITY_GATES.md`](FEASIBILITY_GATES.md)). Record the model, the firmware the TV
reports, the package hash above, and what was observed — not what was expected.

| # | G5 item | What to do on the TV | What counts as the receipt |
| --- | --- | --- | --- |
| 1 | Lightning 3 runs on the Tizen 6.0 engine | Launch the app. | The home screen renders, with its type at couch distance and no canvas fallback. Record any console error verbatim. |
| 2 | Signed `.wgt` installs | Step 3 above. | The install succeeded (not `Invalid certificate chain`), and the app appears in the TV's app list. |
| 3 | USB storage enumerable | Plug in the drive with media on it, then open **Settings** → **Look for drives again**. | The Settings screen's *Drives the TV reports* row names the drive and its state. **Record the exact label** — the label is platform-dependent and the app deliberately does not hard-code one. |
| 4 | AVPlay opens a local file, seeks, reports duration, pauses and resumes | Play any file. | Picture and audio; the position readout advances; rewind/fast-forward jump a minute; pause and resume work. |
| 5 | D-pad focus reachable, visible, and restored | Arrow through the grid, Enter into a file, Back out, and check the cursor returns where it was. | Focus is visible from the sofa at every step, and Back returns to the same card. Back at the home screen leaves the app rather than doing nothing. |
| 6 | State survives restart and a power cycle | Play part of a file, note the position, exit the app, relaunch, then power the TV off and on. | The same file is offered to continue from the same position after both. |
| 7 | What a reinstall destroys | After the above, reinstall and re-check. | Written down, honestly, so nobody is surprised later. |

### The three open questions, in the order they will bite

Everything below is a place where the code makes a documented choice that **cannot be
confirmed from a Mac**, and each one names the observable that settles it. Finding one
wrong is a successful result, not a failure.

1. **The URI form AVPlay will accept.** `platform/media-roots.ts` builds `file://` +
   the absolute path, with the relative half validated by the same rule the Rust core
   uses. Samsung's examples pass a *plain* absolute path, and some firmware accepts
   both. **Observable:** if AVPlay reports an error on a path that resolves, try the
   plain form — `playbackUri()` is the single function to change, and it is the only
   place that decides.
2. **The storage label and mount path.** `candidateRoots`/`listMediaRoots` take the
   platform's own answer; the app resolves the file through `tizen.filesystem` rather
   than guessing `/USB`. **Observable:** the Settings row from checklist item 3.
3. **AVPlay's own state transitions.** Every call is guarded by `getState()` and only
   made in the states Samsung documents, because a thrown player call inside a key
   handler takes the screen down. **Observable:** the Settings screen reports the live
   state string; any deviation shows up as a position that stops advancing or a file
   that never starts.

### Not implemented in this client yet — gaps, not bugs

Stated plainly so they are not mistaken for working features:

- **No catalogue population on the TV.** The client can *see* a drive but does not yet
  walk it: there is no TypeScript equivalent of the Rust scanner, so no files are
  catalogued and Home and Browse show their (correct, tested) empty states. **This is
  the next piece of TV work**, and the storage, progress and focus halves it will need
  are already built and tested.
- **No external subtitle files**, and no on-screen audio/text track menu. Embedded text
  tracks are drawn by the decoder itself. The track *reading* is implemented and tested
  (`platform/avplay-pure.ts`); the menu is not, because a menu that cannot be shown to
  work is worse than no menu.
- **The remote's Info key opens Settings**, not a context menu, and the media keys are
  *requested* with `registerKey` — the Settings screen reports how many the TV actually
  accepted, which is the only honest source for that number.
- **Android TV, LG webOS and the Tizen emulator are out of scope**, and the emulator in
  particular proves nothing about playback: it does not implement AVPlay's real
  decoding.

---

## 5. When the TV is attached, the build is not the unknown

The toolchain is installed, the profile is active, the bundle is one file, the widget
stages itself, and the package is signed and hash-recorded. G5's remaining work is on
the television — starting with whether its DUID is in the distributor certificate.
