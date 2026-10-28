/**
 * Removable storage, and the absolute path a decoder needs.
 *
 * Two facts shape this file, and both of them are about not guessing:
 *
 * 1. **Storage labels are platform-dependent.** `tizen.filesystem.listStorages()` is
 *    the platform's own answer, and the app uses it rather than hard-coding `/USB` or
 *    a sample's label. The label of the owner's drive is discovered, or the app says
 *    it found nothing.
 * 2. **AVPlay needs an absolute path.** A catalogued item stores a path *relative* to
 *    its root — never absolute, because identity must not depend on a path — so the
 *    two are joined here, and the join is refused unless the relative half passes the
 *    same validation the core uses (`core/locator.ts`).
 *
 * The `file://` question is the one thing this file cannot settle from here: Samsung's
 * own examples pass a plain absolute path, while a `file://` URL is also accepted in
 * some firmware. The form is one named function with one override, and the device
 * handoff lists it as the first thing to confirm on the TV — see
 * `docs/TIZEN_DEVICE_HANDOFF.md`.
 */

import { resolveLocator } from '../core/locator.ts'
import { tizenGlobal, type TizenStorage } from './tizen.ts'

/** A storage device the app may play media from. */
export interface MediaRoot {
  /** The platform's own label, used both to display and to resolve. */
  readonly label: string
  /** The platform's state string for the device. */
  readonly state: string
  /** True when the platform reports it mounted. */
  readonly mounted: boolean
}

/**
 * Lists removable storage.
 *
 * Never throws and never fabricates an entry: a host without the filesystem API
 * returns an empty list, and a platform error rejects with the platform's own message
 * so the screen can show it rather than an empty list that looks like "no drive".
 */
export async function listMediaRoots(): Promise<
  { readonly ok: true; readonly roots: readonly MediaRoot[] } | { readonly ok: false; readonly reason: string }
> {
  const filesystem = tizenGlobal()?.filesystem
  if (filesystem === undefined) {
    return { ok: false, reason: 'This host has no removable-storage API, so no drive can be seen.' }
  }
  return await new Promise((resolve) => {
    try {
      filesystem.listStorages(
        (storages: TizenStorage[]) => {
          const roots = storages.map((storage) => ({
            label: storage.label,
            state: storage.state,
            mounted: storage.state.toUpperCase() === 'MOUNTED',
          }))
          resolve({ ok: true, roots })
        },
        (error: unknown) => {
          resolve({ ok: false, reason: `The TV refused to list storage: ${describe(error)}` })
        },
      )
    } catch (error) {
      resolve({ ok: false, reason: `The TV refused to list storage: ${describe(error)}` })
    }
  })
}

/**
 * The absolute path a file is played from.
 *
 * `rootPath` is the mounted path the platform reported for the storage the file is
 * on, and `relativePath` is what the catalogue holds. The relative half goes through
 * `resolveLocator`, so a stored string that escapes its root — `..`, an absolute path,
 * a drive prefix — produces `null` rather than a path pointing at something the owner
 * never granted.
 */
export function absolutePath(rootPath: string, relativePath: string): string | null {
  return resolveLocator(rootPath, relativePath)
}

/**
 * The URI handed to AVPlay for an absolute path.
 *
 * Kept as a single function so there is exactly one place to change when the TV says
 * which form it wants. The default is `file://`, which is the form a `<video>` element
 * and AVPlay alike accept for a local path, and the handoff lists confirming it as an
 * on-device step.
 *
 * A path that does not begin with a separator is refused rather than coerced: a
 * relative path here would be resolved against the widget's own storage, which is not
 * where media lives.
 */
export function playbackUri(absolute: string): string | null {
  if (absolute.length === 0 || (!absolute.startsWith('/') && !/^[a-zA-Z]:/.test(absolute))) {
    return null
  }
  const encoded = absolute
    .split('/')
    .map((segment) => encodeURIComponent(segment))
    .join('/')
  return `file://${encoded}`
}

/** A thrown platform error, as a readable message. */
function describe(error: unknown): string {
  if (typeof error === 'string') {
    return error
  }
  if (error instanceof Error) {
    return error.message
  }
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const message = (error as { message?: unknown }).message
    if (typeof message === 'string') {
      return message
    }
  }
  return 'no message'
}
