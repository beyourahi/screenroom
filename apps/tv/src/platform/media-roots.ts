/**
 * Removable storage: what the TV has, how to read it, and what to hand the decoder.
 *
 * Two facts shape this file, and both come from Samsung's own documentation rather
 * than from guessing:
 *
 * 1. **Storage is a *location*, not a path.** `tizen.filesystem.listStorages()` returns
 *    `FileSystemStorage` objects with a `label`, a `type` (`INTERNAL` or `EXTERNAL`) and
 *    a `state` (`MOUNTED`, `REMOVED`, `UNMOUNTABLE`) — and the reference notes that for
 *    external storages the label *is* the location to use, spelled `removable_…`. So
 *    the app stores the platform's own label and never invents `/USB` or `sda1`.
 * 2. **The platform owns the URI.** `FileSystemManager.toURI(path)` is the API whose
 *    documentation says a URI identifies a file for playback. Building `file://…` by
 *    hand is therefore a *fallback*, used only when the platform will not answer, and
 *    which one was used is reported rather than assumed.
 *
 * What this file deliberately does not do is write. The app never modifies, moves or
 * deletes the owner's media: there is no `deleteFile`, no `createDirectory`, no
 * `openFile(…, 'w')` anywhere in the client, and the widget requests read-only
 * privileges to match.
 */

import { scannableStorage } from '../core/scan.ts'
import { tizenGlobal, type TizenStorage } from './tizen.ts'

/** A storage device the TV reports. */
export interface MediaRoot {
  /** The platform's own label, used to display, to scan and to resolve. */
  readonly label: string
  /** `INTERNAL` or `EXTERNAL`, as the platform spells it. */
  readonly type: string
  /** `MOUNTED`, `REMOVED` or `UNMOUNTABLE`, as the platform spells it. */
  readonly state: string
  /** True when the platform reports it mounted. */
  readonly mounted: boolean
  /** True when the platform calls it removable. */
  readonly removable: boolean
  /** True when this app reads media from it. */
  readonly scannable: boolean
}

/** What asking the platform for its storages produced. */
export type RootsResult =
  | { readonly ok: true; readonly roots: readonly MediaRoot[] }
  | { readonly ok: false; readonly reason: string }

/**
 * One entry a directory listing produced.
 *
 * `isDirectory` is the platform's own answer when it gave one, and `null` when it did
 * not. That distinction is load-bearing: `null` means "ask again", `false` means "this is
 * a file", and treating the second as the first would catalogue a folder as a video.
 */
export interface ReadEntry {
  /** The entry's own name, or the full path when the platform only gave that. */
  readonly name: string
  /** What the platform said this is, or `null` when it would not say. */
  readonly isDirectory: boolean | null
}

/** What listing one directory produced. */
export type DirectoryResult =
  | { readonly ok: true; readonly entries: readonly ReadEntry[] }
  | { readonly ok: false; readonly reason: string }

/** What turning a location into something the decoder accepts produced. */
export type UriResult =
  | { readonly ok: true; readonly uri: string; readonly via: 'toURI' | 'file-url' }
  | { readonly ok: false; readonly reason: string }

/**
 * Lists the storages the TV reports.
 *
 * Never throws and never fabricates an entry: a host without the filesystem API returns
 * a truthful failure, and a platform error is passed on with the platform's own message
 * so a screen can show it rather than an empty list that looks like "no drive".
 *
 * Every storage is reported, including the internal ones and the virtual roots, because
 * the Settings screen's job is to say what the *TV* said. Which of them this app will
 * scan is a separate, tested decision (`scannableStorage`).
 */
export async function listMediaRoots(): Promise<RootsResult> {
  const filesystem = tizenGlobal()?.filesystem
  if (filesystem === undefined) {
    return { ok: false, reason: 'This host has no removable-storage API, so no drive can be seen.' }
  }
  return await new Promise<RootsResult>((resolve) => {
    const onError = (error: unknown): void => {
      resolve({ ok: false, reason: `The TV refused to list storage: ${describe(error)}` })
    }
    try {
      filesystem.listStorages((storages: TizenStorage[]) => {
        const roots = (Array.isArray(storages) ? storages : []).map((storage) => {
          const type = typeof storage.type === 'string' ? storage.type : ''
          const state = typeof storage.state === 'string' ? storage.state : ''
          const label = typeof storage.label === 'string' ? storage.label : ''
          return {
            label,
            type,
            state,
            mounted: state.toUpperCase() === 'MOUNTED',
            removable: type.toUpperCase() === 'EXTERNAL',
            scannable: scannableStorage({ label, type, mounted: state.toUpperCase() === 'MOUNTED' }),
          }
        })
        resolve({ ok: true, roots })
      }, onError)
    } catch (error) {
      onError(error)
    }
  })
}

/**
 * Normalizes whatever a listing callback handed back into entries.
 *
 * The W3C Filesystem API — the one Tizen implements, and the one Samsung's TV reference
 * points at — calls back with an array of **`File` objects**, not names: each has `name`,
 * `isFile`, `isDirectory`, `path` and `toURI()`. A previous version of this file filtered
 * for `typeof === 'string'`, which threw away every `File` and made each folder look
 * empty — a scan of a full drive would have reported zero files. Both shapes are accepted
 * here, and a `File`'s own `isDirectory` flag is kept rather than re-derived.
 *
 * Anything that is neither a string nor an object with a usable name is dropped: an
 * unnamed entry cannot be joined to a path, and inventing one would be a false receipt.
 */
export function readEntriesFrom(list: unknown): ReadEntry[] {
  if (!Array.isArray(list)) {
    return []
  }
  const entries: ReadEntry[] = []
  for (const item of list) {
    if (typeof item === 'string') {
      if (item.length > 0) {
        entries.push({ name: item, isDirectory: null })
      }
      continue
    }
    if (item === null || typeof item !== 'object') {
      continue
    }
    const record = item as { name?: unknown; fullPath?: unknown; isDirectory?: unknown; isFile?: unknown }
    const name = typeof record.name === 'string' && record.name.length > 0 ? record.name : null
    if (name === null) {
      continue
    }
    entries.push({
      name,
      isDirectory:
        typeof record.isDirectory === 'boolean'
          ? record.isDirectory
          : record.isFile === true
            ? false
            : null,
    })
  }
  return entries
}

/**
 * Lists one directory's entries.
 *
 * Two platform calls are tried, in the order the documentation makes authoritative:
 *
 * 1. `filesystem.resolve(path, …)` then `File.listFiles(…)` — the documented way to turn
 *    a virtual location (which is what a storage label is) into a browsable folder.
 * 2. `filesystem.listDirectory(path, …)` — the Tizen device-API spelling, kept only as a
 *    fallback because not every firmware ships one, and the walker must not depend on
 *    the richer of the two existing.
 *
 * Which of them was used is not reported to the screen; a failure is. An empty folder and
 * an unreadable one are different answers and stay different.
 */
export async function readDirectory(path: string): Promise<DirectoryResult> {
  const filesystem = tizenGlobal()?.filesystem
  if (filesystem === undefined) {
    return { ok: false, reason: 'This host cannot list a folder.' }
  }
  return await new Promise<DirectoryResult>((resolve) => {
    const onError = (error: unknown): void => {
      resolve({ ok: false, reason: describe(error) })
    }
    const done = (entries: unknown): void => {
      resolve({ ok: true, entries: readEntriesFrom(entries) })
    }

    // The fallback keeps the *first* reason when it has nothing of its own to say, so a
    // firmware that answers neither call reports why `resolve` refused rather than a
    // generic "no way to list", which would hide a permission error.
    const legacy = (firstReason?: unknown): void => {
      if (typeof filesystem.listDirectory !== 'function') {
        onError(firstReason ?? new Error('the TV has no way to list a folder'))
        return
      }
      try {
        filesystem.listDirectory(path, done, onError)
      } catch (error) {
        onError(error)
      }
    }

    if (typeof filesystem.resolve !== 'function') {
      legacy()
      return
    }
    try {
      filesystem.resolve(
        path,
        (folder: { listFiles?: (ok: (files: unknown) => void, fail: (error: unknown) => void) => void }) => {
          if (typeof folder?.listFiles !== 'function') {
            legacy()
            return
          }
          try {
            folder.listFiles(done, onError)
          } catch (error) {
            onError(error)
          }
        },
        // A storage label is a location `resolve` is documented to accept, but a firmware
        // that disagrees must not turn a readable drive into "would not list": the other
        // spelling gets one try, and only then is the refusal reported.
        (error: unknown) => legacy(error),
        'r',
      )
    } catch (error) {
      onError(error)
    }
  })
}

/**
 * Whether a path is a folder, or `null` when the host cannot say.
 *
 * `null` is not a failure to hide: the walker needs it to tell "the platform answered
 * no" from "the platform has no way to answer", and it treats the second as a reason to
 * count the entry as unreadable rather than as a file.
 */
export function isDirectoryPath(path: string): boolean | null {
  const filesystem = tizenGlobal()?.filesystem
  if (filesystem === undefined || typeof filesystem.isDirectory !== 'function') {
    return null
  }
  try {
    return filesystem.isDirectory(path) === true
  } catch {
    return null
  }
}

/** True when the path exists, or `null` when the host cannot say. */
export function pathExists(path: string): boolean | null {
  const filesystem = tizenGlobal()?.filesystem
  if (filesystem === undefined || typeof filesystem.pathExists !== 'function') {
    return null
  }
  try {
    return filesystem.pathExists(path) === true
  } catch {
    return null
  }
}

/**
 * The URI to hand the decoder for a location.
 *
 * `toURI` is the platform's own answer and is preferred: it is the API whose
 * documentation says the URI identifies a file for playback, and it removes this
 * app's guess about whether AVPlay wants `file:///USB/…` or `/USB/…`. Samsung's own
 * examples pass a plain absolute path, which is exactly why the question is left to
 * the platform instead of to a string concatenation here.
 *
 * The fallback exists for hosts without the API and is *named* in its result, so a
 * device report can tell which path was taken. A location that is not rooted in
 * anything (an empty string, a bare relative name) is refused rather than coerced:
 * resolving it would land inside the widget's own storage, which is not where media
 * lives.
 */
export function fileUri(location: string): UriResult {
  if (location.length === 0) {
    return { ok: false, reason: 'The file has no location to open.' }
  }
  const filesystem = tizenGlobal()?.filesystem
  if (filesystem !== undefined && typeof filesystem.toURI === 'function') {
    try {
      const uri = filesystem.toURI(location)
      if (typeof uri === 'string' && uri.length > 0) {
        return { ok: true, uri, via: 'toURI' }
      }
    } catch {
      // Falls through to the attempt below rather than failing the playback outright.
    }
  }
  // The fallback is only usable for a location that is an absolute device path. A
  // platform location (`removable1/…`) has no absolute spelling this app can invent —
  // prefixing `file://` to it would produce a URL that looks right and points nowhere,
  // which is exactly the kind of silent wrong answer the handoff's first open question
  // exists to prevent. So it is refused, with the reason.
  if (!location.startsWith('/') && !/^[a-zA-Z]:/.test(location)) {
    return {
      ok: false,
      reason:
        'The TV would not convert this drive location into a file address, and this app will not guess one.',
    }
  }
  const encoded = location
    .split('/')
    .map((segment) => encodeURIComponent(segment))
    .join('/')
  return { ok: true, uri: `file://${encoded}`, via: 'file-url' }
}

/** What a storage change notification reported, or why there will be none. */
export type StorageWatch =
  | { readonly watching: true; readonly stop: () => void }
  | { readonly watching: false; readonly reason: string }

/**
 * Watches for a drive being plugged in or pulled out.
 *
 * This is the difference between a library that goes stale in silence and one that
 * marks a drive's rows unreadable the moment the drive leaves. Nothing is deleted when
 * it fires — the owner's positions are not the platform's to withdraw — the rows only
 * become unavailable.
 */
export function watchStorageChanges(
  onChange: (storage: { readonly label: string; readonly state: string }) => void,
): StorageWatch {
  const filesystem = tizenGlobal()?.filesystem
  if (filesystem === undefined || typeof filesystem.addStorageStateChangeListener !== 'function') {
    return { watching: false, reason: 'This host does not report storage changes.' }
  }
  try {
    const watchId = filesystem.addStorageStateChangeListener((storage: TizenStorage) => {
      onChange({
        label: typeof storage?.label === 'string' ? storage.label : '',
        state: typeof storage?.state === 'string' ? storage.state : '',
      })
    })
    return {
      watching: true,
      stop: () => {
        try {
          filesystem.removeStorageStateChangeListener?.(watchId)
        } catch {
          // A listener the platform has already dropped needs no further handling.
        }
      },
    }
  } catch (error) {
    return { watching: false, reason: describe(error) }
  }
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
