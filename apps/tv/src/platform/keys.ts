/**
 * The remote control, as a table of facts.
 *
 * A Samsung TV delivers arrow keys, Enter and Back by default; every other key on the
 * remote has to be requested, and only some models support each one. So the codes and
 * the names are kept here in one place, the registration is attempted once and its
 * result is reported, and an unrecognised key is a no-op rather than a guess.
 *
 * Two things this file deliberately does *not* do:
 *
 * * it does not assume a code works. `getSupportedKeys()` is the platform's own
 *   answer, and the app records what registration actually returned.
 * * it does not map a key to an action. Codes become names here; what a name *means*
 *   is per screen, and lives in the screen (`App` for global keys).
 *
 * The media codes are the ones Samsung publishes for its TV remotes; the arrow,
 * Enter and Back codes are what the engine already receives. Back is `10009` — the
 * engine's default map does not include it, so without the entry below the single most
 * pressed key on the remote would do nothing at all.
 */

/** A remote key, named by what it means rather than by its code. */
export type RemoteKey =
  | 'left'
  | 'right'
  | 'up'
  | 'down'
  | 'enter'
  | 'back'
  | 'playPause'
  | 'play'
  | 'pause'
  | 'stop'
  | 'rewind'
  | 'fastForward'
  | 'info'
  | 'channelUp'
  | 'channelDown'

/** Samsung remote key names, as `tvinputdevice.registerKeyBatch` wants them. */
export const TIZEN_KEY_NAMES: Readonly<Record<RemoteKey, string | null>> = {
  // Arrows, Enter and Back arrive without registration.
  left: null,
  right: null,
  up: null,
  down: null,
  enter: null,
  back: null,
  playPause: 'MediaPlayPause',
  play: 'MediaPlay',
  pause: 'MediaPause',
  stop: 'MediaStop',
  rewind: 'MediaRewind',
  fastForward: 'MediaFastForward',
  info: 'Info',
  channelUp: 'ChannelUp',
  channelDown: 'ChannelDown',
}

/** The keymap handed to `Blits.Launch`, merged over the engine's own defaults. */
export const REMOTE_KEYMAP: Readonly<Record<number, string>> = {
  // Already in the engine's default map, repeated so this table is the whole truth.
  37: 'left',
  39: 'right',
  38: 'up',
  40: 'down',
  13: 'enter',
  8: 'back',
  27: 'escape',
  // The Samsung remote's own codes.
  10009: 'back',
  10252: 'playPause',
  415: 'play',
  19: 'pause',
  413: 'stop',
  412: 'rewind',
  417: 'fastForward',
  457: 'info',
  427: 'channelUp',
  428: 'channelDown',
}

/** What registration did, reported rather than assumed. */
export interface KeyRegistration {
  /** The keys the platform accepted. */
  readonly registered: readonly string[]
  /** The keys the platform refused, with the reason it gave. */
  readonly refused: readonly { readonly key: string; readonly reason: string }[]
  /** True when the host has the input-device API at all. */
  readonly supported: boolean
}

/**
 * Requests the remote keys the engine does not receive by default.
 *
 * Registration is per key and can fail for a key the model does not have, so each
 * attempt is recorded on its own. A refusal is not fatal: the arrow keys still work,
 * and the app knows which of its own advertised controls are unreachable.
 */
export function registerRemoteKeys(): KeyRegistration {
  const device = globalThis as { tizen?: { tvinputdevice?: { registerKey(k: string): void } } }
  const inputdevice = device.tizen?.tvinputdevice
  if (inputdevice === undefined) {
    return { registered: [], refused: [], supported: false }
  }

  const registered: string[] = []
  const refused: { key: string; reason: string }[] = []
  for (const name of Object.values(TIZEN_KEY_NAMES)) {
    if (name === null) {
      continue
    }
    try {
      inputdevice.registerKey(name)
      registered.push(name)
    } catch (error) {
      refused.push({ key: name, reason: describe(error) })
    }
  }
  return { registered, refused, supported: true }
}

/** A thrown platform error, as a readable string. */
function describe(error: unknown): string {
  if (error instanceof Error) {
    return error.message
  }
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const message = (error as { message?: unknown }).message
    return typeof message === 'string' ? message : 'no message'
  }
  return String(error)
}

/** True when a key name is one this app knows how to act on. */
export function isRemoteKey(name: string): name is RemoteKey {
  return name in TIZEN_KEY_NAMES
}
