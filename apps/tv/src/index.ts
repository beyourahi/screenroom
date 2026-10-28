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
import { recordEvidence } from './platform/evidence.ts'
import { REMOTE_KEYMAP } from './platform/keys.ts'

/**
 * Records anything that would otherwise only reach a console nobody can read.
 *
 * A retail TV gives this Mac no log, no screenshot and no inspector, so an uncaught
 * error is indistinguishable from a blank screen with the app still "running" — which
 * is exactly the first install's failure. These two listeners turn that silence into a
 * line in the app's own storage, readable after an `sdb pull`.
 *
 * Registered before `Blits.Launch`, so a failure *during* launch is caught too, and
 * the handlers never throw: an error reporter that breaks the app it reports on is
 * worse than no reporter.
 */
function recordFailures(): void {
  const target = globalThis as {
    addEventListener?: (type: string, listener: (event: unknown) => void) => void
  }
  if (typeof target.addEventListener !== 'function') {
    return
  }
  target.addEventListener('error', (event) => {
    const detail = event as { message?: string; filename?: string; lineno?: number; error?: unknown }
    const message = detail.message ?? (detail.error instanceof Error ? detail.error.message : 'error')
    const where = detail.filename ? ` at ${detail.filename.split('/').pop()}:${detail.lineno ?? 0}` : ''
    recordEvidence('error', `${message}${where}`)
  })
  target.addEventListener('unhandledrejection', (event) => {
    const reason = (event as { reason?: unknown }).reason
    const message = reason instanceof Error ? reason.message : String(reason)
    recordEvidence('rejection', message)
  })
}

/**
 * Records what the renderer actually produced, shortly after launch.
 *
 * The canvas is the evidence that Lightning mounted and the GPU surface exists: a blank
 * screen with a correctly sized canvas and no `error` entry is a drawing problem, while
 * a blank screen with no canvas is a launch problem, and those have different fixes.
 * The check is deferred because the element is created during launch.
 */
function recordRender(): void {
  globalThis.setTimeout(() => {
    try {
      const canvases = Array.from(document.querySelectorAll('canvas'))
      const first = canvases[0]
      const painted = document.getElementById('app')?.childElementCount ?? 0
      const ratio = globalThis.devicePixelRatio ?? 1
      // The viewport is recorded because the attribute size alone does not say whether
      // what was drawn *fits on the screen*: a 1920-wide canvas in a smaller CSS
      // viewport shows only its top-left corner, which is exactly how the first working
      // build appeared on the set.
      const viewport = `${globalThis.innerWidth ?? 0}x${globalThis.innerHeight ?? 0}`
      recordEvidence(
        'render',
        first === undefined
          ? `canvases=0 mounted=${painted} viewport=${viewport} dpr=${ratio}`
          : [
              `canvases=${canvases.length}`,
              `size=${first.width}x${first.height}`,
              `css=${first.style.width || 'unset'}/${first.style.height || 'unset'}`,
              `client=${first.clientWidth}x${first.clientHeight}`,
              `viewport=${viewport}`,
              `dpr=${ratio}`,
              `mounted=${painted}`,
            ].join(' '),
      )
    } catch (error) {
      recordEvidence('render', `probe failed: ${error instanceof Error ? error.message : 'unknown'}`)
    }
  }, 700)
}

recordFailures()

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

recordRender()
