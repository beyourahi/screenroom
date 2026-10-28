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

import { tizenGlobal } from './tizen.ts'

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

/** The Tizen system info the app may read, when the host provides it. */
export interface DeviceInfo {
  /** The TV model code, as the platform reports it, or `null`. */
  readonly model: string | null
  readonly firmwareVersion: string | null
  readonly platformVersion: string | null
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
  const avplay = tizen?.webapis?.avplay !== undefined
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

/** Reads what the platform says about the device. Every field degrades to `null`. */
export function readDeviceInfo(): DeviceInfo {
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
    model: read('http://tizen.org/system/model_name'),
    firmwareVersion: read('http://tizen.org/system/build_version'),
    platformVersion: read('http://tizen.org/system/platform.version'),
  }
}
