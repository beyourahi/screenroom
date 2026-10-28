import { afterEach, test } from 'node:test'
import assert from 'node:assert/strict'

import { detectHost, hostSummary, readDeviceInfo } from '../src/platform/env.ts'
import { hasAvPlay } from '../src/platform/tizen.ts'

/**
 * These tests pin the one platform fact the first installed build got wrong on the
 * owner's television: `webapis` is a **sibling** of `tizen` on `window`, not a member of
 * it. Reading `tizen.webapis.avplay` returns `undefined` on a real TV, so the app
 * concluded it was a browser and sent playback to a `<video>` element that cannot open
 * a local file there. The set's own boot record read `avplay=0 filesystem=1`, which is
 * the contradiction these tests exist to prevent.
 */

const globals = globalThis as { tizen?: unknown; webapis?: unknown }

function install(shape: { tizen?: unknown; webapis?: unknown }): void {
  globals.tizen = shape.tizen
  globals.webapis = shape.webapis
}

afterEach(() => {
  delete globals.tizen
  delete globals.webapis
})

/** The minimum a TV provides: device APIs on `tizen`, media APIs on `webapis`. */
const tizenDeviceApis = {
  filesystem: { listStorages() {} },
  tvinputdevice: { registerKey() {}, registerKeyBatch() {}, getSupportedKeys: () => [] },
}

const avplayStub = {
  open() {},
  close() {},
  getState: () => 'NONE',
  getCurrentTime: () => 0,
  getDuration: () => 0,
  getTotalTrackInfo: () => [],
  setDisplayRect() {},
  prepareAsync() {},
  play() {},
  pause() {},
  stop() {},
  seekTo() {},
}

test('AVPlay is found on window.webapis, not on window.tizen', () => {
  install({ tizen: tizenDeviceApis, webapis: { avplay: avplayStub } })
  assert.equal(hasAvPlay(), true)
  assert.equal(detectHost().avplay, true)
})

test('a TV with AVPlay is reported as the TV, not as a browser preview', () => {
  install({ tizen: tizenDeviceApis, webapis: { avplay: avplayStub } })
  const host = detectHost()
  assert.equal(host.kind, 'tizen')
  assert.match(hostSummary(host), /AVPlay is present/)
  // Nothing about the report may suggest a preview when the TV is live.
  assert.equal(host.notes.some((note) => note.includes('desktop browser')), false)
})

test('the nested form is still accepted, for hosts that expose it that way', () => {
  install({ tizen: { ...tizenDeviceApis, webapis: { avplay: avplayStub } } })
  assert.equal(hasAvPlay(), true)
})

test('a host with tizen device APIs but no AVPlay is a browser, and says so', () => {
  // This is exactly what the owner's TV reported before the fix: `filesystem=1` with
  // `avplay=0`. Reporting it as a browser is then correct — the bug was the reading.
  install({ tizen: tizenDeviceApis })
  const host = detectHost()
  assert.equal(host.avplay, false)
  assert.equal(host.kind, 'browser')
  assert.match(hostSummary(host), /AVPlay is absent/)
})

test('the device identity comes from webapis.productinfo', () => {
  install({
    tizen: tizenDeviceApis,
    webapis: {
      avplay: avplayStub,
      productinfo: {
        getModel: () => 'UA50AU8000',
        getFirmware: () => 'T-KTMAKUC-1490.3',
        getVersion: () => '6.0',
      },
    },
  })
  assert.deepEqual(readDeviceInfo(), {
    model: 'UA50AU8000',
    firmwareVersion: 'T-KTMAKUC-1490.3',
    // `getVersion()` is the product version and must not be read as the Tizen version.
    platformVersion: null,
    productVersion: '6.0',
  })
})

test('a product API that throws degrades to null rather than failing the probe', () => {
  install({
    webapis: {
      avplay: avplayStub,
      productinfo: {
        getModel: () => {
          throw new Error('not supported')
        },
      },
    },
  })
  assert.equal(readDeviceInfo().model, null)
})

test('an empty product answer is treated as no answer', () => {
  install({ webapis: { avplay: avplayStub, productinfo: { getModel: () => '' } } })
  assert.equal(readDeviceInfo().model, null)
})
