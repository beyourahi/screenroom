# Samsung TV device handoff

**Target:** the owner's Samsung **UA50AU8000RXER** — a 2021 Crystal UHD running the
Tizen 6.0 web engine. This is the only TV in scope
([ADR-004](ADR-004_PERSONAL_EDITION_SCOPE.md)).

**State (2026-10-29, second session): the owner's drive was scanned and 1,200 files were
catalogued and stored — and the library still could not be seen or browsed.** Two causes,
both fixed in this tree; §2a records them. The app now draws the whole catalogue on Home,
with a persistent rail and a D-pad-only path to every screen. The set was **not** touched
in this session (the owner's background-first policy): every claim below is from the
desktop browser preview and the test suite, which is **not** a TV receipt.

**Last installed package (2026-10-29, first session):** 160,893 bytes,
sha256 `69a794b2fb1ef4452b805cc4ebd3afc561fe9b2ae324a8943dc5dcfbe7ed7e14`, installed with
`install completed` and launched; the set's own record reads
`boot host=tizen avplay=1 filesystem=1 inputdevice=1 storage=1 keys=9/refused=0
model=UAU8000 firmware=T-NKLAUABC-2310.0` plus
`render canvases=1 size=1920x1080 css=1920px/1080px client=1920x1080 viewport=1920x1080 dpr=1
mounted=1`. That build **predates** the library and navigation fixes, so the owner's
1,200-file report describes it, not this tree. **Items 3–12 are open: no file has played
on the set and no navigation flow has been exercised there.**

The rule it exists to follow: a packaged `.wgt`, a signed package, a Tizen Studio
install, a simulated filesystem or a television emulator is **not** a TV test. Only the
television is.

---

## 1. Verified without the TV (2026-10-28)

```bash
cd apps/tv
pnpm install                 # Blits 2.15.0, TypeScript 5.9, Vite 7
pnpm typecheck               # app source and tests, separately
pnpm test                    # 189 tests, 0 failures
pnpm build                   # typecheck + Vite + the Tizen staging step
```

| Check | Result (2026-10-28) |
| --- | --- |
| `pnpm typecheck` | clean. Application source typechecks with **no ambient types at all** (`"types": []`), so a Node-only global cannot typecheck inside a component that has to run in a web widget; the tests are the only files that see Node's globals |
| `pnpm test` | **189 tests, 0 failures** — the focus machine, the progress guard, the store and its durability rules, the scanner's rules, a whole-drive walk over a simulated Tizen filesystem, filename interpretation, the track menu and next-episode rule, decoder preferences, the remote-key wiring of every screen, relative-locator validation, AVPlay's display-rect maths and track reading, the design tokens and the card geometry. Per-suite counts are in [`TEST_PLAN.md`](TEST_PLAN.md) |
| `pnpm build` | one 462,142-byte `main.js` and a 1,391-byte `index.html`; no code splitting (a widget has no server to fetch a second chunk from) |
| `apps/tv/build/widget/` | the staged widget: `config.xml`, `icon.png`, `index.html`, `main.js`, and the Tizen project metadata. The staging step refuses to continue if the bundle is unlinked, missing, or referenced by an absolute path |
| Signed package | **built and installed.** `apps/tv/build/ScreenroomTV-0.1.0.wgt`, **160,893 bytes, sha256 `69a794b2…7e14` (2026-10-29, carries the directory-listing fix)**; the build before it was 160,680 bytes, sha256 `01d60176…d2231`; earlier builds were 157,202 → 157,986 → 158,079 → 158,101 → 158,462 → 158,500 bytes as the device defects below were fixed. The hash changes with every one of those; quote the one the TV actually has (see section 2) |
| The packaged bundle | byte-identical to the tested one, checked on every build: the `main.js` digest inside the `.wgt` equals the digest of `dist/main.js` printed by the staging step |
| Package contents | exactly `author-signature.xml`, `config.xml`, `icon.png`, `index.html`, `main.js`, `signature1.xml` — nothing stray, and three privileges (`tv.inputdevice`, `filesystem.read`, `productinfo`) in the manifest |
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

### 1b. Re-verified after the library and navigation fixes (2026-10-29)

| Check | Result |
| --- | --- |
| `pnpm typecheck` | clean (app source and tests, separately) |
| `pnpm test` | **228 tests, 0 failures** — 189 before the library/navigation work, plus the rail and cursor arithmetic, the shelf rule, the grid window, the search screen's keyboard and matches, and the template guards (a reference that does not resolve; a loop variable named `key`) |
| `pnpm build` | typecheck + Vite + the Tizen staging step, one `main.js` of **541,802 bytes** (sha256 `3e46079b…4d85`), no code splitting |
| `apps/tv/build/widget/` | staged: `config.xml`, `icon.png`, `index.html`, `main.js`, and the project metadata |
| Browser preview (`vite`, 1920×1080) | Home draws the catalogue (`1200 files in the library`, `Row 3 of 240` after two presses of Down); the rail is focusable and opens every destination; Movies shows 600 of them; Search types a query and lists matches; Media Sources and Settings draw and scroll; Detail opens the chosen card; Play reaches the player and reports honestly that this host cannot convert a drive location into a file address |
| Signed package | **not rebuilt in this session.** The `.wgt` on the set is the older one; packaging is the next step and its hash must be recorded here when it exists |

**This is not a TV receipt.** The preview has no AVPlay, no USB storage, no remote, and
no Tizen engine. It proves layout, the navigation arithmetic and that failure copy is
truthful — nothing more.

### What that does **not** mean

- It does **not** mean the TV will accept the install. The distributor certificate in
  the `screenroom` profile must list *this television's* DUID. The TV rejects anything
  else with `Invalid certificate chain`.
- It does **not** mean the bundle runs. Nothing in this project has ever executed on a
  Tizen engine: not the renderer, not WebGL, not a single frame.
- It does **not** mean the app plays video, lists a drive or switches a track. No byte
  of media has passed through AVPlay and no USB drive has been walked by the set.

---

## 1a. The set is reachable *intermittently* — check before believing anything below

**2026-10-28 (later in the day):** `sdb devices` listed the set once, then the next
command returned `error: target not found`. `ping 192.168.0.172` is 100 % loss, the Mac
is still on `192.168.0.110/24` with gateway `192.168.0.1`, and the ARP table holds no
entry for the television. That is one of three states and no other: the set is off, it
has left this network, or it has left developer mode.

The single command that settles it once the set is awake, with nothing else typed first:

```bash
CLI=$(ls ~/.codex/plugins/cache/tizen-platform/tizen-sdk-skills/*/lib/cli/remote-device-cli.js | sort -V | tail -1)
node "$CLI" connect 192.168.0.172:26101     # must report `device`, not `offline`
```

If the address changed, the router's client list is the place to read it — the set's
hostname or MAC, not a fresh `/24` sweep.

---

## 2. The set was reached (2026-10-28, supersedes the blocked report below)

The television is on and in developer mode. `remote-device-cli connect` returned
`connected` at `192.168.0.172:26101`, and `sdb capability` identifies it as
`vendor_name:Samsung`, `profile_name:tv`, `platform_version:6.0`, `cpu_arch:armv7`.

The distributor certificate was then checked **without printing certificate material
or the DUID**: the package's chain is Samsung-issued, its leaf carries exactly one
`deviceid` binding, and the DUID the set reports through `cert-manager acquire-duid`
matches it. That is why the install was accepted rather than rejected with
`Invalid certificate chain`.

### What this firmware does *not* give a developer, and what replaced it

| Wanted | Available? | Consequence |
| --- | --- | --- |
| `sdb shell 0 <anything>` | **No** — a short whitelist only (`applist`, `getduid`, `vd_applist`, `vd_appinstall`, `was_execute`) | no `ps`, no file listing, no `dmesg` |
| Screenshot (`screenshot`, `capture`, `vd_screenshot`) | **No** | the screen cannot be photographed from a Mac |
| `sdb dlog` over TCP | **No** — returns 0 lines | no console output |
| Remote Web Inspector on port 9222 | **No** — `app_launcher` is absent, so an app cannot be debug-launched | no CDP, no DOM inspection |
| Key injection (`sendkey`) | **No** | the remote must be pressed by a person |

**The one channel that works is the app's own storage.** `sdb pull
/opt/usr/home/owner/apps_rw/PlYByk3psw/data` returns the widget's Chromium profile,
including `Local Storage/leveldb`, and the client writes a bounded evidence record
there (`core/evidence.ts`). A `boot` line and a `render` line are what turned "it
launched" into `host=tizen avplay=1 … canvases=1 size=1920x1080 mounted=1`.

```bash
CLI=~/.codex/plugins/cache/tizen-platform/tizen-sdk-skills/1.6.1/lib/cli/remote-device-cli.js
node "$CLI" connect 192.168.0.172:26101
SDB=~/.local/share/screenroom-toolchains/tizen/tools/sdb
"$SDB" -s 192.168.0.172:26101 push apps/tv/build/ScreenroomTV-0.1.0.wgt \
  /home/owner/share/tmp/sdk_tools/ScreenroomTV-0.1.0.wgt
# `sdb install` only pushes on this firmware; the TV needs vd_appinstall.
"$SDB" -s 192.168.0.172:26101 shell 0 vd_appinstall PlYByk3psw.ScreenroomTV \
  /home/owner/share/tmp/sdk_tools/ScreenroomTV-0.1.0.wgt
"$SDB" -s 192.168.0.172:26101 shell 0 was_execute PlYByk3psw.ScreenroomTV
# read the set's own record:
"$SDB" -s 192.168.0.172:26101 pull /opt/usr/home/owner/apps_rw/PlYByk3psw/data /tmp/sr-tv
strings -a /tmp/sr-tv/data/Local\ Storage/leveldb/* | grep -E "^host=|canvases="
```

### The blocked report this replaces, kept for the audit trail

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

**Observed on 2026-10-29, and it contradicts the usual claim:** the reinstall of this
build did **not** clear the widget's data. The `boot` and `render` lines written by the
previous day's build (epoch `1793160…`) were still in `Local Storage/leveldb` after the
new one had booted (`1793211…`). So on this firmware, installing over the app via
`vd_appinstall` preserved the data directory — recorded as an observation of one
reinstall, not a guarantee, and item 12 still has to be signed off by the owner.

---

## 2a. Why 1,200 catalogued files were invisible (2026-10-29)

The owner's report was precise and the pipeline was innocent: **the scan worked, the
catalogue was built, and the document was written.** `library` in the evidence record
read `items=1200 available=1200 write=accepted`, and Media Sources on the set showed
`1200 files catalogued from 1 drive` / `Last scan: 1200 files found on 1 drive, with
nothing skipped`. Four defects sat between that store and the screen, in this order:

1. **Home drew only *Continue watching*.** `Home.ts` called `continueWatching(items)`,
   which is empty until something has been played. `emptyState()` correctly returns
   `null` for a library that has files and no filter, so the screen drew a heading, a
   sub-heading, and nothing else — a successful scan that looks exactly like a failed
   one. A library that is only visible after you have already watched something is a
   library that cannot be started.
2. **Nothing navigated anywhere.** The rail did not exist. `/browse`, `/movies`,
   `/series`, `/search` and `/sources` were either absent or unreachable, and the only
   route off Home was bound to the remote's **Info** key (`input.info → $router.to('/settings')`)
   — a key most Samsung remotes do not label and this set's own key registration
   cannot create. The catalogue was reachable only by pressing a button the viewer does
   not have, which is why Settings "required a special remote button".
3. **The grid was a page, not a window.** Browse drew a page of rows turned by the
   rewind/forward media keys — again, keys nothing else needed, and a page that ended
   mid-row.
4. **The search screen's keyboard drew nothing.** `:for="key in $keys"` with
   `key="$key.id"` compiles to a collision: Blits assigns the loop variable to the
   scope slot named after it, and the `key` attribute then overwrites that slot with the
   id. The body saw a string where it expected an object, every key was created with an
   undefined position and size, and half the screen was empty with **no error and no
   warning**. Found by rendering the screen in the preview and looking at it; guarded
   now by a template test that forbids a loop variable named `key`.

What replaced them, and what each fix is proven by:

| Defect | Fix | Proven by |
| --- | --- | --- |
| Home showed only resume points | `homeShelf()` in `core/browse.ts`: resume points when there are any, **the whole catalogue** when there are not | `browse.test.ts` cases on both branches; the preview draws all 1,200 |
| No navigation | `core/nav.ts` (rail entries, two-zone cursor, grid window) + `components/Rail.ts` drawn on Home, Movies, TV Shows, Search, Media Sources and Settings | `nav.test.ts` (movement, clamping, windowing), `rail.test.ts`, and a preview walk of every destination |
| Paging by media keys | `gridWindow()`: Down is always the next row, the band slides, only ten cards exist | `nav.test.ts` window cases; preview shows `Row 3 of 240` and scrolls |
| Search keyboard invisible | loop variable renamed; template test added | the screen renders 39 keys and types into a real query in the preview |

Two smaller defects were found by walking the built screens and are fixed with the same
commit: the grid kept a focused card **while the rail held the cursor** (two inverted
surfaces, so Enter's target was ambiguous), and OK on the destination already open was a
dead key on Search, Media Sources and Settings.

**What is still a Mac-only claim.** Everything above is verified in the browser preview
and the test suite. The preview has no AVPlay, no USB storage and no remote, so it
proves layout, navigation arithmetic and honest failure copy — nothing about the set.

---

## 3. Install and run

```bash
node ~/.codex/plugins/cache/tizen-platform/tizen-sdk-skills/*/lib/cli/install-app-cli.js \
  <path>/build/ScreenroomTV-0.1.0.wgt --run
```

(`tizen-install-app` owns this; nothing here should be typed as a raw `sdb install`.)

The app must come up as **Screenroom**. It opens on the rail, with **Home** highlighted,
and everything is reachable with the arrow keys and OK — no Info key, no colour keys, no
media keys. The rail is always on screen on the library screens:

```
Home · Movies · TV Shows · Search · Media Sources · Settings
```

First useful action on the set: **Down** to **Media Sources** and **OK**, then **OK** on
*Scan for media*. Settings prints, from the host itself: TV model, software version,
Tizen version, playback path (AVPlay or `<video>`), how many remote keys registration
accepted, where state is stored, every drive the TV reports with its type/state and
whether this app will scan it, and what the last scan did.

Back is the remote's Return key everywhere: it closes the player menu, then leaves the
file; on the home screen it leaves the app. While a file is playing, **Up** or **Down**
opens the control menu (pause, 10-second seek, tracks, next episode, back to the
library) — the media keys remain shortcuts, never requirements.

---

## 4. The acceptance checklist

Each numbered item is a separate receipt for gate G5
([`FEASIBILITY_GATES.md`](FEASIBILITY_GATES.md)). Record the model, the firmware the TV
reports, the package hash above, and what was observed — not what was expected.

**Where this stands (2026-10-29).** Item **2 passes** (installed, and the certificate
provably authorises this set). Item **1 passes**: the engine runs the bundle, the TV
reports `canvases=1 size=1920x1080 mounted=1` with no JS errors, and the **owner has seen
the home screen with readable text and cards**. Items **3–12 are open** and every one of
them needs the remote in a hand, because this firmware accepts no synthetic key press.

The owner's USB drive holds **movies and shows in nested folders**, not loose files at the
drive root. That is what the walker is built for: it is breadth-first and descends to 16
levels (`MAX_DEPTH` in `core/scan.ts`), skips hidden folders, and `folderHintOf` strips a
`Season NN` folder so the show's own folder is the identity an episode joins. Deeper than
16 levels is reported as `depth_limit` rather than silently dropped.

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
| 10 | D-pad focus reachable, visible, restored | Walk the rail with Down and OK: Home → Movies → TV Shows → Search → Media Sources → Settings, and back out with Return. Then arrow through a grid, Enter into a file, Back out. **Do not press Info, a colour key or a media key at any point** — if the walk needs one, the navigation is not finished. | One highlighted row or card at all times, never two; every destination opens; Left from the first card reaches the rail and Right returns to the same card; Back returns to the same card and leaves the app only from Home. |
| 10b | The whole catalogue is visible and browsable | From Home, press Down until the grid has scrolled a few rows, then OK on a card. | Home shows the scanned files with titles and years; the row counter advances as the band scrolls; the chosen card opens its own detail screen. |
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
- **No screen needs a key the remote may not have.** Settings, Media Sources, Search and
the library are on the rail; the player's controls are in a menu opened by Up/Down. The
media and Info keys are *requested* with `registerKey` and remain shortcuts (Info
toggles the player overlay), and the Settings screen reports how many the TV accepted,
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
