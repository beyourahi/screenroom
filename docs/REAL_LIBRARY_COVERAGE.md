# Real-library coverage (private SSD, 2026-10-29)

> Scope: the owner's own USB SSD library, used as the test source for the macOS
> application. **Nothing in this file names a title or a path from that library.**
> The raw logs — per-file probe output, per-file playback rows and the two
> `*_check` receipts — live in `.artifacts/ssd-test/` on this machine only, are
> Git-ignored, and are never uploaded. This file records *what was exercised and
> what it proved*, which is the part that must survive the logs.
>
> The drive was read-only throughout: no file created, modified, renamed, moved or
> deleted on it, no thumbnail, database, subtitle, log or temporary file written to
> it, and no permissions or mounts touched. Re-checked after the run: the newest
> modification time under the media root is older than the session, and the video
> file count is unchanged.

## Library shape

| Measure | Value |
| --- | --- |
| Video files discovered (read-only `find`, `._*` / backups / hidden dirs excluded) | **830** (729 MKV, 101 MP4) |
| Total size | 941.1 GiB |
| Show folders | 19 |
| Films | 291 |
| Episodes matched by `SxxEyy` | 539 |
| External sidecar subtitles | 21 |
| Indexed into the isolated test database | **830 rows → 291 films, 18 series, 53 seasons, 539 episodes, 0 unknown** |
| Probed with ffprobe | **830** (827 readable, 3 truncated MP4s) |
| Playback-tested offscreen through the real worker | **9** files across the codec matrix |

## Capability matrix

`PASS` = exercised on this library and satisfied · `FAIL` = exercised and broken
(then fixed, with the regression test named) · `SKIP` = could not be answered by this
library, with the reason · `MANUAL` = needs a visible window, a person's ears or the
physical drive in hand.

| Capability | Status | Evidence |
| --- | --- | --- |
| Discovery of every eligible file | PASS | 830 found, `visited=1002 imported=830 issues=0` |
| AppleDouble `._*` companions excluded | PASS | was 851 rows / 21 junk titles; fixed, `scanner.rs` test `apple_double_companions_are_not_media` |
| Full index (works, seasons, episodes, editions) | PASS | 830 rows, 291 films, 18 series, 53 seasons, 539 episodes, 0 unknown |
| Film/series identification against an independent count | PASS | every show's episode count equals its folder count (e.g. 149/149, 72/72, 40/40, 30/30, 26/26, 26/26); no film absorbed into a series |
| Series year identity | PASS | was 0 of 18 with a year; fixed, `filename.rs` |
| Duplicate detection and rescan stability | PASS | a second scan produced 830 editions, 0 duplicates, all posters kept, 2.31 s |
| Bounded scan, memory | PASS | first run 3.59 s, peak RSS 8.2 MB |
| Cancellation keeping what it found | PASS | `core/tests/scanner.rs`; the app's Cancel calls the same token |
| Paging over the whole catalogue | PASS | 830 unique rows in 2 pages, 0 repeated (`library_check`) |
| Paging refuses an oversized page | PASS | `invalid_page` (`library_check`) |
| Ordering (Title / Added / Year / Progress) | PASS | all four return a full page; Title verified ascending over 200 rows |
| Search (title, filename, literal wildcards, no-match) | PASS | 1 row for an exact title, 30 for a word, 0 for `%`, 0 for nonsense |
| Kind partition | PASS | 291 films + 539 episodes + 0 unknown = 830; unknown kind refused |
| Season grouping per work | PASS | seasons read from a real series; a season page holds only its own episodes |
| Genre filter | SKIP | a scanned library stores no genres until enrichment exists; the picker is now **hidden** until genres exist (it was an always-empty control) |
| Watched filter and one card per work | PASS (fixtures) | Browse groups episodes into their series and filters watched/unwatched; desktop tests, grouped page timed at 10k |
| Available-only filter | PASS | 830 of 830 with the drive mounted |
| Resume point from a checkpoint, and Continue Watching | PASS | checkpoint at 120 s became a Continue Watching row |
| Stale seek generation refused | PASS | a pre-seek generation wrote nothing; the post-seek one was accepted |
| Favourites and watchlist, per profile | PASS | 1 favourite, 1 watchlisted, 0 after clearing |
| Local recommendations | PASS | 2 suggestions, 2 distinct works, each with a reason |
| Preferences across a reopen | PASS | scale 115 %, autoplay off, volume 45 % all survived |
| Profiles add / rename / remove | PASS | round-tripped on the test copy |
| Backup of the real catalogue | PASS | 1 490 944 bytes, 830 files, 309 works |
| Restore into a fresh library | PASS | 830 of 830 rows; a restore over data keeps a safety copy |
| Stored posters are real images | PASS | 21 read back and decoded by ffprobe as JPEG at their declared size, 180 731 bytes, 0 rejected |
| Artwork candidates point at readable files | PASS | 200 of the 288 remaining works checked, 0 missing paths |
| Fresh extraction from real media | PASS | 5 posters extracted, stored and re-decoded, ~1.2 s each |
| A work with an unreadable file | PASS | one truncated MP4 (`moov atom not found`) reported honestly; the app records it and stops asking |
| Playback: HEVC 10-bit MKV | PASS | 4.5 s to first frame, 23.7 fps, audio clock 4.01/4.00 s, seek 88 ms |
| Playback: 4K into a 1080p-height target | PASS | 1920×800 render target, 24.0 fps |
| Playback: AV1 MKV | PASS | opens in 0.4 s |
| Playback: external sidecar SRT | PASS | offered as an `external=true` track and selected |
| Playback: H.264 High 10 + DTS | PASS | 24.0 fps |
| Playback: MPEG-4 | PASS | 24.0 fps |
| Playback: 4 audio tracks, switching | PASS | switched to the last track and read it back |
| Playback: anamorphic content | PASS | 1920×1080 SAR 9:8 now renders at its display size, 1920×960, with no bars inside the frame (was 1920×1080 with 60 px bars, then letterboxed again by the window) |
| Playback: a corrupt MP4 | PASS | `decode_failed — unrecognized file format`, no crash, no fake success |
| Subtitle style (scale/position) reaching the decoder | PASS | read back from libmpv on every file that has subtitles |
| Stored durations / runtimes before first play | PASS | background ffprobe pass: **827 of 830** stored in ~80 s; the 3 truncated MP4s stay unknown; repeated by the packaged bundle under launchd's minimal `PATH` |
| Embedded cover art as poster source | PASS | a file's `attached_pic` stream is the poster (400×600 from real files); otherwise a frame at a real 10 % (`examples/poster_check`) |
| Hardware decoding (4K, battery) | SKIP | `hwdec=no` by design for the CPU bridge; the gate is still open |
| On-screen frame rate, resume after quit, full screen, keyboard journey | MANUAL | every number is offscreen; needs a permissioned visible-window session |
| Audio heard, not measured | MANUAL | gate G3b needs ears |
| macOS feature journeys (first run, relink, restore failure) | PASS | three application-level tests on isolated folders, `apps/desktop/src/app.rs` |

## Reproducing

```bash
# read-only inventory and probe (writes only to .artifacts/)
scripts/probe-library.sh "/Volumes/<volume>/<library>" .artifacts/ssd-test/probe-all.jsonl

# catalogue contracts, on a COPY of the scanned database
cargo run --release -p screenroom-core --example library_check -- .artifacts/ssd-test/checks/contract.db

# poster audit; `fresh` also extracts a few new ones
cargo run --release -p screenroom-core --example artwork_check -- .artifacts/ssd-test/library.db
cargo run --release -p screenroom-core --example artwork_check -- .artifacts/ssd-test/checks/fresh.db fresh

# the app's own poster path (cover first, else a frame), on a COPY
cargo run --release -p screenroom-desktop --example poster_check -- .artifacts/ssd-test/checks/posters.db 20

# bounded real playback, offscreen and muted
cargo run --release -p screenroom-player --example play_file -- <file>...
```

`library_check` **writes** (a favourite, a resume row, a preference, a throwaway
profile, a restore) — always give it a copy, never the library the app is using.

## The rule this file keeps

A file that ffprobe reads is not a file that plays; a play that plays is not a
poster; a stored poster is not a drawn poster; and none of them is a frame on a
screen. Each row above is written at the strength of its evidence and no higher.
