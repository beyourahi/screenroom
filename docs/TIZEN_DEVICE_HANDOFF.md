# Samsung TV device handoff

**Target:** the owner's Samsung **UA50AU8000RXER** — a 2021 Crystal UHD running the
Tizen 6.0 web engine. This is the only TV in scope
([ADR-004](ADR-004_PERSONAL_EDITION_SCOPE.md)).

**State: gate G5 is BLOCKED on the device, and the client is now a real local-media
client off-device.** It lists the TV's drives, walks them, catalogues what it finds,
plays a file through AVPlay with an audio/subtitle menu, remembers a position and
offers the next episode. None of that has happened on a television, and this document
does not pretend otherwise.

The rule it exists to follow: a packaged `.wgt`, a signed package, a Tizen Studio
install, a simulated filesystem or a television emulator is **not** a TV test. Only the
television is.

---

## 1. Verified without the TV (2026-10-28)

```bash
cd apps/tv
pnpm install                 # Blits 2.15.0, TypeScript 5.9, Vite 7
pnpm typecheck               # app source and tests, separately
pnpm test                    # 161 tests, 0 failures
pnpm build                   # typecheck + Vite + the Tizen staging step
```

| Check | Result (2026-10-28) |
| --- | --- |
| `pnpm typecheck` | clean. Application source typechecks with **no ambient types at all** (`"types": []`), so a Node-only global cannot typecheck inside a component that has to run in a web widget; the tests are the only files that see Node's globals |
| `pnpm test` | **161 tests, 0 failures** — the focus machine, the progress guard, the store and its durability rules, the scanner's rules, a whole-drive walk over a simulated Tizen filesystem, filename interpretation, the track menu and next-episode rule, decoder preferences, the remote-key wiring of every screen, relative-locator validation, AVPlay's display-rect maths and track reading, the design tokens and the card geometry. Per-suite counts are in [`TEST_PLAN.md`](TEST_PLAN.md) |
| `pnpm build` | one 462,142-byte `main.js` and a 1,391-byte `index.html`; no code splitting (a widget has no server to fetch a second chunk from) |
| `apps/tv/build/widget/` | the staged widget: `config.xml`, `icon.png`, `index.html`, `main.js`, and the Tizen project metadata. The staging step refuses to continue if the bundle is unlinked, missing, or referenced by an absolute path |
| Signed package | **built.** `apps/tv/build/ScreenroomTV-0.1.0.wgt`, 157,202 bytes, sha256 `b7d82c49755ae1501bffe62040cd2263aaa181510d477d3c4196d8b5710ecc51` |
| The packaged bundle | byte-identical to the tested one: `main.js` sha256 `9b7cbdaea4a9bc2b5b3b3855d6382593742a751032c8153b3c917176bae597c4` inside the `.wgt` and in `dist/` |
| Package contents | exactly `author-signature.xml`, `config.xml`, `icon.png`, `index.html`, `main.js`, `signature1.xml` — nothing stray, and two privileges (`tv.inputdevice`, `filesystem.read`) in the manifest |
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
emulator only.

### What that does **not** mean

- It does **not** mean the TV will accept the install. The distributor certificate in
  the `screenroom` profile must list *this television's* DUID. The TV rejects anything
  else with `Invalid certificate chain`.
- It does **not** mean the bundle runs. Nothing in this project has ever executed on a
  Tizen engine: not the renderer, not WebGL, not a single frame.
- It does **not** mean the app plays video, lists a drive or switches a track. No byte
  of media has passed through AVPlay and no USB drive has been walked by the set.

---

## 2. Device discovery, attempted and reported (2026-10-28)

The previous session's connection could not be recovered from here. What was checked,
through the toolchain that is already configured rather than by hand:

| Check | Result |
| --- | --- |
| `remote-device-cli list-saved` | **0 saved remote devices.** Device Manager's list is empty, so there is no stored address to retry |
| `remote-device-cli scan` | swept `192.168.0.0/24` on SDB port 26101 (3 s per host): **0 devices found** |
| `sdb devices` | `List of devices attached` with nothing under it |

So the set is **off**, on **another network**, or **out of developer mode**. Those three
are the whole list, and the owner can settle each in one step:

1. Switch the TV on and connect it to the same network as this Mac.
2. TV → **Settings → General → Network → Expert settings → IP Settings**, and note the
   address; the app's SDB port is 26101.
3. TV → **Apps → Settings (or the `12345` code) → Developer mode: on**, with this Mac's
   IP entered under "Host PC IP". The TV restarts once.
4. Then, from this repo:

```bash
BASE="$HOME/.codex"
CLI=$(ls "$BASE"/plugins/cache/tizen-platform/tizen-sdk-skills/*/lib/cli/remote-device-cli.js | sort -V | tail -1)
node "$CLI" connect <tv-ip>:26101      # verified: the entry must read `device`, not `offline`
```

Nothing global is changed by any of this, and no television setting is modified by the
app.

**Before the first install:** reinstall on Tizen Studio is an uninstall followed by an
install, and `localStorage` for the widget goes with it — profiles, positions and scan
results included. Take a copy of anything worth keeping first, and see item 7.

---

## 3. Install and run

```bash
node ~/.codex/plugins/cache/tizen-platform/tizen-sdk-skills/*/lib/cli/install-app-cli.js \
  <path>/build/ScreenroomTV-0.1.0.wgt --run
```

(`tizen-install-app` owns this; nothing here should be typed as a raw `sdb install`.)

The app must come up as **Screenroom**. First useful action on the set: open
**Settings** with the remote's **Info** button. It prints, from the host itself: TV
model, software version, Tizen version, playback path (AVPlay or `<video>`), how many
remote keys registration accepted, where state is stored, every drive the TV reports
with its type/state and whether this app will scan it, and what the last scan did.

---

## 4. The acceptance checklist

Each numbered item is a separate receipt for gate G5
([`FEASIBILITY_GATES.md`](FEASIBILITY_GATES.md)). Record the model, the firmware the TV
reports, the package hash above, and what was observed — not what was expected.

| # | G5 item | What to do on the TV | What counts as the receipt |
| --- | --- | --- | --- |
| 1 | Lightning 3 runs on the Tizen 6.0 engine | Launch the app. | The home screen renders, with its type at couch distance and no canvas fallback. Record any console error verbatim. |
| 2 | Signed `.wgt` installs | Step 3 above. | The install succeeded (not `Invalid certificate chain`), and the app appears in the TV's app list. Record the DUID only if it must be added to a profile — never into Git. |
| 3 | USB storage enumerable | Plug in the drive with media, open **Settings**. | The *Drives the TV reports* row names the drive, its type, its state and whether this app scans it. **Record the exact label** — it is platform-dependent and the app deliberately does not hard-code one. |
| 4 | A drive is **walked and catalogued** | Settings → **Scan for media**. | *Last scan* changes from "No scan has run yet." to a file count with a drive count. Home and Browse then show the files. **If the report says the drive would not list, that is permission, not an empty drive** — record the message verbatim (see open question 2). |
| 5 | Identities and progress survive a rescan | Play part of a film, Back out, return to Settings, scan again. | The same film is still offered at the same position; the file count does not double. |
| 6 | AVPlay opens a local file, seeks, reports duration, pauses and resumes | Play any file. | Picture and audio; the position readout advances; rewind/fast-forward jump a minute; pause and resume work. |
| 7 | The track menu works | During playback, press **Up**. | The menu lists audio tracks, *Subtitles off* and the subtitle tracks with the current choice marked. Choosing one changes the audio or subtitles, and the **next** file opens with that choice. |
| 8 | A sidecar subtitle file is used | Put `Film.srt` beside `Film.mkv`, with the extension in the set Samsung documents (`.smi`, `.srt`, `.sub`, `.ttxt`). | Either subtitles appear, drawn by the app, or the overlay says the decoder refused the file — both are receipts. `.ass`/`.ssa` are deliberately not offered. |
| 9 | Next episode | Finish an episode whose next episode is catalogued. | The overlay offers it by season/episode and title; Enter plays it from the beginning. With a gap in the numbering, nothing is offered. |
| 10 | D-pad focus reachable, visible, restored | Arrow through the grid, Enter into a file, Back out. | Focus is visible from the sofa at every step, and Back returns to the same card. Back at the home screen leaves the app rather than doing nothing. |
| 11 | State survives restart and a power cycle | Play part of a file, exit, relaunch, then power the TV off and on. | The same file is offered from the same position after both, and the scan result is still on Settings. |
| 12 | What a reinstall destroys | After the above, reinstall and re-check. | Written down, honestly, so nobody is surprised later. |

### The five open questions, in the order they will bite

Each is a place where the code makes a documented choice that **cannot** be confirmed
from a Mac, and each names the observable that settles it. Finding one wrong is a
successful result, not a failure.

1. **Whether the TV grants `filesystem.read` on a removable drive.** The manifest asks
   for read access and nothing else; the app never writes. If `listDirectory` returns a
   permission error, the scan report says so verbatim and the drive is *not* withdrawn —
   **observable:** checklist item 4 with the exact message. If it is a permission
   refusal, the next thing to try is Samsung's
   `http://tizen.org/privilege/mediastorage`, which is the media-specific grant and
   also carries write access the app does not use. Do not add it pre-emptively; a
   privilege a widget does not need is a privilege that cannot be abused.
2. **The URI form AVPlay will accept.** `platform/media-roots.ts` now prefers
   `tizen.filesystem.toURI(location)` — the platform's own answer — and falls back to
   `file://` only for an absolute device path, *naming which path was taken* in the
   result. A platform location such as `removable1/…` is refused rather than
   `file://`-prefixed, because that would be a URL that looks right and points nowhere.
   **Observable:** if AVPlay reports an error on a location that resolves, the fallback
   and its ordering are the single function to change.
3. **The storage label and how a location addresses a drive.** The app stores the
   platform's own label (`removable1`, whatever the set reports) and never invents
   `/USB` or `sda1`; a platform location is accepted as a root by `core/locator.ts`,
   with a test. **Observable:** the Settings drive row from item 3.
4. **Whether `setExternalSubtitlePath` is honoured for files outside the widget's own
   storage.** Samsung's subtitle sample offers the file before `prepareAsync`, which is
   what this client does, and `onsubtitlechange` is read only when a sidecar was
   accepted — an embedded track is drawn by the decoder and drawing it again would show
   it twice. **Observable:** checklist item 8.
5. **AVPlay's own state transitions.** Every call is guarded by `getState()` and only
   made in the states Samsung documents, because a thrown player call inside a key
   handler takes the screen down. **Observable:** any deviation shows up as a position
   that stops advancing or a file that never starts.

### Known limitations, stated rather than discovered later

- **A renamed file loses its stored position.** The desktop recognises a rename with a
  content fingerprint (head and tail of the file); a TV directory listing reports no
  size, and reading a hundred megabytes off a USB drive to identify a film is not work a
  television should do. `core/scan.ts` says so where the rule lives.
- **`.ass`/`.ssa` subtitles are not offered**, because Samsung's own player does not
  render them. Embedded text tracks are drawn by the decoder; software subtitles from a
  sidecar file are drawn by this app.
- **The walk is bounded** at 25,000 entries and 16 levels, yields to the event loop
  every 20 folders, and can be stopped with Enter on the scan row. Hitting a bound makes
  the scan *incomplete*, and an incomplete scan withdraws nothing.
- **Internal storage and virtual roots are reported but never walked** — a film does not
  live on the TV's flash, and walking it would catalogue the platform's bookkeeping.
- **The remote's Info key opens Settings**, not a context menu, and the media keys are
  *requested* with `registerKey`; the Settings screen reports how many the TV accepted,
  which is the only honest source for that number.
- **LG webOS, Android TV and the Tizen emulator are out of scope.** The emulator in
  particular proves nothing about playback: it does not implement AVPlay's real
  decoding.

---

## 5. When the TV is attached, the build is not the unknown

The toolchain is installed, the profile is active, the bundle is one file, the widget
stages itself, the package is signed and hash-recorded, and the scan, the catalogue and
the player's controls are implemented and covered by off-device tests. G5's remaining
work is on the television — starting with whether it can be reached at all, and then
whether its DUID is in the distributor certificate.
