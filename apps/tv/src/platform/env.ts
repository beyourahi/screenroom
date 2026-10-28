/**
 * What the host this bundle is running on can actually do.
 *
 * This file exists to keep the app honest. The same bundle runs on the Samsung TV and
 * in a desktop browser for a UI preview, and those two hosts support completely
 * different playback. Reporting which one is live — and saying plainly that a browser
 * run proves nothing about the TV — is the difference between a preview and a false
 * device receipt (`docs/FEASIBILITY_GATES.md`, gate G5).
 *
 * Nothing here guesses. A capability is true only when the object that provides it is
 * present and usable, and the notes carry the limits the booleans cannot.
 */

import { tizenGlobal, webapisGlobal } from './tizen.ts'

/** Which host the bundle is executing in. */
export type HostKind =
  /** A Tizen web widget: the only host where AVPlay exists. */
  | 'tizen'
  /** Anything else — a desktop browser preview. Not a TV, and not evidence. */
  | 'browser'

/** What this host supports, as observed rather than as assumed. */
export interface HostCapabilities {
  readonly kind: HostKind
  /** True when `webapis.avplay` is present. */
  readonly avplay: boolean
  /** True when the Tizen filesystem API is present, which USB enumeration needs. */
  readonly filesystem: boolean
  /** True when the remote-key API is present, which non-arrow keys need. */
  readonly inputdevice: boolean
  /** True when `localStorage` accepted a probe write. Probed, not assumed. */
  readonly durableStorage: boolean
  /** The user agent, kept verbatim so a receipt can quote it. */
  readonly userAgent: string
  /** Things a reader must not infer from the booleans above. */
  readonly notes: readonly string[]
}

/** The device's own identity, as the host reports it. Every field degrades to `null`. */
export interface DeviceInfo {
  /** The TV model, as the platform reports it (e.g. `UAU8000`), or `null`. */
  readonly model: string | null
  readonly firmwareVersion: string | null
  /**
   * The Tizen platform version, from `tizen.systeminfo` only.
   *
   * It is deliberately *not* filled from `productinfo.getVersion()`: on the owner's set
   * that call answers `3.7`, which is the television's product version, and printing it
   * under a "Tizen version" label would be a false receipt on a set whose engine is
   * Tizen 6.0. A field that cannot be answered stays `null` and says so.
   */
  readonly platformVersion: string | null
  /** The product version the Samsung API reports, which is a different thing. */
  readonly productVersion: string | null
}

/**
 * Probes whether `localStorage` really works.
 *
 * Presence is not enough: a web widget can expose `localStorage` and still refuse
 * every write, because the quota is full or because the platform blocked it. The only
 * honest test writes one key and reads it back.
 */
function probeLocalStorage(): boolean {
  try {
    const key = '__screenroom_probe__'
    globalThis.localStorage.setItem(key, '1')
    const read = globalThis.localStorage.getItem(key) === '1'
    globalThis.localStorage.removeItem(key)
    return read
  } catch {
    return false
  }
}

/** Reads the host capabilities. Called once, and its result is what Settings shows. */
export function detectHost(): HostCapabilities {
  const tizen = tizenGlobal()
  // AVPlay lives on `window.webapis`, not on `window.tizen` — reading the wrong one
  // reported `avplay=0` on the owner's television and sent playback to a `<video>`
  // fallback that cannot open a local file on a TV.
  const avplay = webapisGlobal()?.avplay !== undefined
  const filesystem = tizen?.filesystem !== undefined
  const inputdevice = tizen?.tvinputdevice !== undefined
  const durableStorage = probeLocalStorage()
  const kind: HostKind = tizen !== null && avplay ? 'tizen' : 'browser'

  const notes: string[] = []
  if (kind === 'browser') {
    notes.push(
      'This is a desktop browser, not the TV. Playback here uses a <video> element and proves nothing about AVPlay.',
    )
  }
  if (!avplay) {
    notes.push('AVPlay is absent, so local file playback cannot be exercised on this host.')
  }
  if (!filesystem) {
    notes.push('The Tizen filesystem API is absent, so USB storage cannot be enumerated here.')
  }
  if (!inputdevice) {
    notes.push('The Tizen input-device API is absent, so media keys will not arrive here.')
  }
  if (!durableStorage) {
    notes.push('Storage refused a probe write. Positions and settings cannot be kept on this host.')
  }

  return {
    kind,
    avplay,
    filesystem,
    inputdevice,
    durableStorage,
    userAgent: globalThis.navigator?.userAgent ?? '',
    notes,
  }
}

/**
 * What platform-specific behaviour is in use right now, phrased for a person.
 *
 * Shown in Settings. It is worded so that nobody can read a browser result as a TV
 * result: the sentence names the host, and a browser host says so.
 */
export function hostSummary(capabilities: HostCapabilities): string {
  if (capabilities.kind === 'tizen' && capabilities.avplay) {
    return 'Samsung TV: AVPlay is present. Local playback uses the TV decoder.'
  }
  return 'Desktop browser preview: AVPlay is absent, so nothing here proves TV playback.'
}

/**
 * Reads what the platform says about the device. Every field degrades to `null`.
 *
 * The source is `webapis.productinfo`, which is the Samsung API that actually answers
 * on a TV; the Tizen `systeminfo` property names this code used first return nothing on
 * the owner's set (its own boot record read `model=unknown`). `systeminfo` is kept as a
 * fallback rather than deleted, because a host that has it and no product API is still
 * better served by an answer than by `null`.
 */
export function readDeviceInfo(): DeviceInfo {
  const product = webapisGlobal()?.productinfo
  const fromProduct = (name: 'getModel' | 'getFirmware' | 'getVersion'): string | null => {
    const read = product?.[name]
    if (typeof read !== 'function') {
      return null
    }
    try {
      // Called on the object, not detached: the platform's implementation is a native
      // binding and a bare function reference loses `this`.
      const value = read.call(product)
      return typeof value === 'string' && value.length > 0 ? value : null
    } catch {
      return null
    }
  }

  const systeminfo = tizenGlobal()?.systeminfo
  const read = (key: string): string | null => {
    if (systeminfo === undefined) {
      return null
    }
    try {
      return systeminfo.getPropertyValue(key) ?? null
    } catch {
      // The platform throws for a property it does not implement, which is a fact
      // about the property rather than an error worth surfacing.
      return null
    }
  }
  return {
    model: fromProduct('getModel') ?? read('http://tizen.org/system/model_name'),
    firmwareVersion:
      fromProduct('getFirmware') ?? read('http://tizen.org/system/build_version'),
    platformVersion: read('http://tizen.org/system/platform.version'),
    productVersion: fromProduct('getVersion'),
  }
}
