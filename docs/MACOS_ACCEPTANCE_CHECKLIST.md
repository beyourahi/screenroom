# macOS human-acceptance checklist

Everything here needs a **visible window, a person's eyes or ears, or a drive in
hand**, so none of it can be run under the background-first testing policy. Every
other macOS behaviour is covered headlessly; see `IMPLEMENTATION_STATUS.md`
("macOS audit and completion"). Until a step below is ticked by the owner, its
status is **PENDING**, not passed.

Use the installed bundle, with Wi-Fi off for the whole session:

```bash
scripts/package-macos.sh && scripts/verify-bundle.sh   # must end "All checks passed."
open dist/Screenroom.app
```

Expect two first-launch prompts: Gatekeeper ("unidentified developer": right-click
› Open once), and removable-volume access when the SSD folder is first read. That
access prompt returns after each rebuild, because an ad-hoc signature changes with
every build.

| # | Step | Pass when |
| --- | --- | --- |
| 1 | First run: Settings › Add folder › choose the SSD's library folder | The scan row counts up, cards appear before it finishes, and it ends with "Scan finished". |
| 2 | Library grid after about two minutes | Cards show posters, cover art where a file has one, and runtimes. A series is **one** card. |
| 3 | Open a series, use ↑/↓ to pick episode 3, press Enter | Episode 3 plays, not the episode the screen opened on. |
| 4 | Watch about a minute, press Space, then ←/→ | Pauses and resumes; arrows move ±10 s. **Listen**: speech is in sync with lips. |
| 5 | Audio and subtitle pickers on a file with several tracks | The spoken language and the subtitle text both change. |
| 6 | Press f, then the green button (or ⌃⌘F) to leave fullscreen | The picture fills the screen. The header returns after leaving, and one Escape then exits the player. |
| 7 | Play Tulsa King S01E01 (2:1 anamorphic) | The picture is wide with no extra bars inside it, and nothing is stretched. |
| 8 | Play a 4K HEVC film for five minutes on battery | Smooth motion; note the battery percentage before and after. |
| 9 | Let an episode end | "Up next … in 5 s" appears. Cancel stays on the finished episode, and Play now (or waiting) starts the next. |
| 10 | Mid-film, press ⌘Q; reopen the app | Continue Watching shows the film within a second of where it was. |
| 11 | Mid-film, click Home in the header | Sound stops at once, and the film is in Continue Watching. |
| 12 | Unplug the SSD mid-film, then plug it back in and Rescan | A clear failure, not a hang; cards show "unavailable" and keep their history; after Rescan they play again. |
| 13 | Settings › Back up, change something, Restore that backup (press twice) | The change is undone, and the status names the `…before-restore` safety copy. |
| 14 | Add a second profile and switch to it; quit and reopen | It reopens on that profile. Remove asks twice. |
| 15 | Keyboard only: Home → Library → open → play → Escape → Escape | Every step is reachable, and the focused card is visible and readable. |

Record the result of each step (pass, fail plus what was seen) under the
"Owner session" heading in `IMPLEMENTATION_STATUS.md`. A failed step is a defect to
fix, never a row to delete.
