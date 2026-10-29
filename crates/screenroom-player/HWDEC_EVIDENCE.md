# macOS software-renderer hardware decode check

Run on the owner's Apple M4 MacBook Air with mpv/libmpv 0.41.0 (client API
2.5). Audio was muted. The three files were opened read-only, one process at a
time. Each playback row covers 20 seconds after the first non-blank frame. The
render bridge downsizes large frames to a 1920-pixel maximum dimension; reported
dimensions are the delivered RGBA frame dimensions.

| Sample | Mode requested | First frame (ms) | Delivered fps | Worst gap (ms) | CPU mean / max (%) | RSS mean / max (MiB) | `hwdec-current` | Non-blank | Mean luma |
|---|---:|---:|---:|---:|---:|---:|---|---|---:|
| 4K HEVC Main10 sample A (3240×2160) | no | 2496 | 13.549 | 249.83 | 343.78 / 454.30 | 737 / 743 | no | yes | 0.1839 |
| 4K HEVC Main10 sample A (3240×2160) | videotoolbox-copy | 2137 | 18.149 | 111.74 | 164.48 / 176.80 | 475 / 536 | videotoolbox-copy | yes | 0.1838 |
| 4K HEVC Main10 sample A (3240×2160) | auto-copy | 2103 | 19.449 | 150.41 | 165.08 / 174.60 | 486 / 498 | videotoolbox-copy | yes | 0.1840 |
| 4K HEVC Main10 sample B (3840×1600) | no | 2138 | 23.949 | 51.68 | 65.09 / 82.00 | 551 / 554 | no | yes | 0.7745 |
| 4K HEVC Main10 sample B (3840×1600) | videotoolbox-copy | 2135 | 23.950 | 57.74 | 51.47 / 69.10 | 386 / 391 | videotoolbox-copy | yes | 0.7744 |
| 4K HEVC Main10 sample B (3840×1600) | auto-copy | 2130 | 23.949 | 57.36 | 56.76 / 73.60 | 387 / 393 | videotoolbox-copy | yes | 0.7744 |
| 1080p H.264 sample C (1804×1080) | no | 2300 | 23.949 | 52.44 | 30.13 / 34.30 | 187 / 217 | no | yes | 0.0173 |
| 1080p H.264 sample C (1804×1080) | videotoolbox-copy | 2394 | 23.949 | 53.63 | 20.38 / 26.40 | 135 / 179 | videotoolbox-copy | yes | 0.0173 |
| 1080p H.264 sample C (1804×1080) | auto-copy | 2408 | 23.950 | 51.92 | 20.75 / 24.40 | 150 / 177 | videotoolbox-copy | yes | 0.0173 |

CPU and RSS are one-second `ps -o %cpu,rss -p <pid>` samples. macOS `ps %cpu`
is a process-level percentage, not a battery or energy measurement. The corrected
serial rerun occurred under mixed host load, including unrelated high-CPU
processes, so these are single-run observations rather than controlled energy
measurements. Luma is the mean Rec. 709 weighted RGB value normalized to 0–1
from the sampled 20-second frame PNG. The PNGs are outside Git in the local
evidence directory alongside the raw per-run logs and `results.csv`.

## Decision

Use `auto-copy` by default on macOS; retain `no` as the default on platforms
without measurements. On sample A, automatic copy-back reduced sampled CPU by
52.0% and mean RSS by 34.0%, with effectively identical luma (0.1839 vs
0.1840). It delivered 19.45 fps against the source's 23.976 fps, so it did not
meet real-time delivery in this run. Sample B remained at 23.95 fps while CPU
fell 12.8% and mean RSS fell 29.7%; sample C remained at 23.95 fps while CPU
fell 31.1% and mean RSS fell 19.9%. All sampled images had expected delivered
dimensions and non-black RGB pixels. `auto-copy` selected
`videotoolbox-copy` on this Mac for all three files. Given materially lower CPU
and memory use on all samples, matching luma, and the built-in software fallback,
the macOS default is still useful, with sample A's dropped-frame result as a
known limitation requiring further measurement under controlled load. A
generated MPEG-2 regression case outside mpv's default hardware codec whitelist
asserts that `auto-copy` reports `hwdec-current=no` and still delivers a frame.

Callers can use `Player::start_with_hwdec(HwdecMode::No)` to force software
decoding. `SCREENROOM_HWDEC=no` provides the same override to existing desktop
launches. The existing desktop `Player::start()` call needs no wiring change;
on macOS it now selects the measured policy.

## Idle worker

| Worker | CPU mean / max (%) | RSS mean / max (MiB) | Worker wakeups in 30 s |
|---|---:|---:|---:|
| Before: 20 ms timeout poll | 0.00 / 0.00 | 43.5 / 54.8 | 1,500 timeout wakes (50/s, from the loop interval) |
| After: callbacks + condition variable | 0.00 / 0.00 | 43.4 / 54.1 | 0 notifications during the sample |

Both CPU readings are below `ps`'s observed resolution, so this run does not
establish an idle CPU reduction. It does establish that the updated loop had no
command/event/render notifications during the 30-second idle sample, instead of
its old 1,500 timeout wakeups. The idle example prints the notification count
and samples process CPU/RSS once per second.

## API references

- mpv 0.41 manual, **Video → `--hwdec`**, including `auto-copy`, safe fallback,
  and the `videotoolbox-copy` system-memory description.
- mpv 0.41 manual, **Video → `--hwdec-codecs`** and **Properties →
  `hwdec-current`**. `no` indicates software decoding; the property is
  unavailable if no decoder is loaded.
- `/opt/homebrew/include/mpv/client.h` 0.41: `mpv_event_property` defines
  `MPV_FORMAT_STRING` event payload as pointer-to-pointer; wakeup callback
  section documents draining `mpv_wait_event()` until `MPV_EVENT_NONE`.
- `/opt/homebrew/include/mpv/render.h` 0.41: **Software renderer** describes
  `MPV_RENDER_API_TYPE_SW`; the render update callback is a notification, and
  render API functions must not be called from inside it.

Online manual sections: [Video options (`--hwdec`, `--hwdec-codecs`)](https://mpv.io/manual/stable/#video)
and [Properties (`hwdec-current`)](https://mpv.io/manual/stable/#properties).
