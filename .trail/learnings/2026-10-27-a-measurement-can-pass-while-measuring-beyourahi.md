---
title: A measurement can pass while measuring nothing
author: Beyourahi
date: 2026-10-27
branch: main
id: l-b3de7acd
scope: repo
status: active
src: agent
touches:
  - crates/screenroom-player/examples/render_bench.rs
  - crates/screenroom-player/src/render.rs
  - crates/screenroom-player/src/lib.rs
---

Three separate ways the new offscreen `render_bench` produced a confident number
that was not true. All three were caught only by asking what the number *could*
have come from, so they are worth re-checking in any future benchmark here.

1. The seek-latency check was `position >= target`. After 11 s of playback the
   playhead was already past every target, so the condition was satisfied before
   the seek took effect and the benchmark reported 125 ns to 1.2 us round trips.
   A latency measurement has to assert *arrival at* the target AND that the
   playhead moved (`(position - target).abs() < 0.5 && (before - position).abs()
   > 0.5`), which a backwards seek cannot satisfy early. Real figure: 59-73 ms.

2. `render::MAX_DIMENSION` is 1920, so a 3840x2160 fixture is decoded at 4K and
   then presented at 1920x1080. The run prints `frame size: 1920x1080`, which is
   the only reason it is visible at all. Any 4K claim from this benchmark (or
   from the app) is false until that bound moves. Do not read the 4K row as 4K.

3. libmpv holds `sub-scale` as single precision, so 0.1 reads back as
   `0.10000000149011612`. An `assert_eq!` on a value read back from libmpv is a
   flaky test waiting to happen; compare with a tolerance. This is also why the
   Settings screen shows a whole percentage rather than the read-back number.

The general rule, and the reason the previous 11.5 fps bug cost a full
edit-measure-revert cycle: a number that looks plausible is not evidence. Print
the intermediate it depends on — frame size, the pre-seek playhead, the raw
read-back — so the benchmark can contradict itself when it is wrong.
