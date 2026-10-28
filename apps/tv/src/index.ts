/**
 * The bundle's entry point.
 *
 * The launch settings are sized for the actual device rather than for a development
 * laptop, because every one of them is a decision about a 2021 Tizen web engine:
 *
 * * `w`/`h` are the authored surface the whole app is drawn in, and the TV scales it;
 * * `keymap` adds the Samsung remote's own codes, without which Back and the media keys
 *   are simply not delivered;
 * * `inputThrottle` and `holdTimeout` are set to the values a remote needs — a held
 *   arrow on a TV repeats far faster than a keyboard does, and unthrottled repeats are
 *   how a grid ends up skipping cards;
 * * `gpuMemory` is sized for a TV, not a desktop GPU. The limit is deliberately
 *   conservative: this app draws no poster art yet, and the setting is here so the
 *   first texture-heavy screen added later cannot quietly exhaust the TV's memory.
 *
 * `multithreaded` is left off. It is faster in theory and has been the source of
 * engine-specific instability, and this build's frame budget is not close to needing
 * it.
 */

import Blits from '@lightningjs/blits'

import App from './App.ts'
import { REMOTE_KEYMAP } from './platform/keys.ts'

Blits.Launch(App, 'app', {
  w: 1920,
  h: 1080,
  // 0 = errors only. A TV's log is hard to read at the best of times, and the app
  // reports its own state on the Settings screen instead of relying on console noise.
  debugLevel: 0,
  keymap: REMOTE_KEYMAP,
  inputThrottle: 80,
  holdTimeout: 320,
  screenResolution: '1080p',
  renderQuality: 'high',
  // Transparent so the player's video surface shows through wherever the app draws
  // nothing — the player renders behind the canvas by design (`platform/playback.ts`).
  canvasColor: 'transparent',
  defaultFont: 'sans-serif',
  enableMouse: false,
  multithreaded: false,
  gpuMemory: {
    max: 90,
    target: 0.7,
    cleanupInterval: 3000,
    baseline: 20,
  },
})
