/**
 * Relative-locator validation, mirroring the Rust core's rule.
 *
 * A catalogued item stores a path relative to its source root. That string comes
 * from a scan of the owner's own disk, but it is still the one piece of data that
 * decides which file on the TV gets opened, so it is validated rather than trusted:
 * an absolute path, a `..` segment, a NUL byte, a Windows drive prefix or a root
 * escape would let a stored string reach outside the folder the owner granted.
 *
 * The rejected set is deliberately the same as
 * `crates/screenroom-core/src/library/catalog.rs`, including the separator
 * handling: the TV's own storage reports `/USB/…` style roots, but a file that
 * arrived from another machine's library can carry a backslash, and a backslash is
 * what Windows uses to escape upwards.
 */

/** Why a locator was refused. Named so the UI can say something specific. */
export type LocatorFault =
  | 'empty'
  | 'absolute'
  | 'parent_directory'
  | 'root'
  | 'nul_byte'
  | 'windows_prefix'

/** The result of validating a stored relative locator. */
export type LocatorVerdict =
  | { readonly safe: true; readonly segments: readonly string[] }
  | { readonly safe: false; readonly fault: LocatorFault }

/** A Windows drive, as in `C:`, or a UNC/device prefix, as in `\\\\server`. */
function hasWindowsPrefix(locator: string): boolean {
  return /^[a-zA-Z]:/.test(locator) || locator.startsWith('\\\\') || locator.startsWith('//')
}

/**
 * True when the locator is a plain relative path inside its source root.
 *
 * Returns the cleaned segments on success so callers can join them onto the root
 * themselves instead of re-parsing the string. Empty segments (from a doubled
 * separator) are dropped, so `Shows//Film.mkv` is accepted as `Shows/Film.mkv`
 * rather than refused; that is a spelling difference, not an escape attempt.
 */
export function checkLocator(locator: string): LocatorVerdict {
  if (locator.length === 0) {
    return { safe: false, fault: 'empty' }
  }
  if (locator.includes('\0')) {
    return { safe: false, fault: 'nul_byte' }
  }
  if (hasWindowsPrefix(locator)) {
    return { safe: false, fault: 'windows_prefix' }
  }
  // A leading separator is an absolute path on every platform this client runs on.
  if (locator.startsWith('/') || locator.startsWith('\\')) {
    return { safe: false, fault: 'absolute' }
  }

  const segments: string[] = []
  for (const segment of locator.split(/[/\\]/)) {
    if (segment.length === 0 || segment === '.') {
      continue
    }
    if (segment === '..') {
      return { safe: false, fault: 'parent_directory' }
    }
    segments.push(segment)
  }

  if (segments.length === 0) {
    return { safe: false, fault: 'root' }
  }
  return { safe: true, segments }
}

/**
 * A media root, as the TV actually spells one.
 *
 * Two forms are legitimate, and the difference is the platform's rather than this
 * app's:
 *
 * * **An absolute device path**, as a desktop library imported from elsewhere carries —
 *   `/Volumes/Media`, `/USB/Media`, `D:\Media`.
 * * **A platform location**, which is what a Tizen storage *is*. Samsung's filesystem
 *   reference states that `FileSystemStorage.label` is "used as location parameter",
 *   and names `removable_…` as the location for external storages — so a USB drive is
 *   addressed as `removable1`, not as a mount path this app would have to invent.
 *
 * A root that is neither is refused. That matters more than it looks: a rooted-looking
 * relative path such as `Movies` would join to produce something resolved against the
 * widget's own private storage, which is not where the owner's media lives, and a
 * silent wrong answer there is worse than a refusal.
 */
function usableRoot(rootPath: string): string | null {
  const root = rootPath.replace(/[/\\]+$/, '')
  if (root.length === 0 || root.includes('\0')) {
    return null
  }
  if (root.startsWith('/') || /^[a-zA-Z]:/.test(root)) {
    return root
  }
  // A platform location: a storage label or a virtual root, with no separator in it.
  return /^[A-Za-z0-9_][A-Za-z0-9._-]*$/.test(root) ? root : null
}

/**
 * Joins a validated locator onto a granted root.
 *
 * Returns `null` when either half is unusable, so a caller cannot accidentally use an
 * unvalidated string. Trailing separators on the root are tolerated because the two
 * places a root comes from — a folder picker and a platform label — do not agree on
 * whether one is included.
 *
 * The result is a *location*, which on this platform is what the filesystem API takes:
 * `removable1/Movies/Film.mkv`. Turning it into something a decoder accepts is the
 * platform adapter's job (`tizen.filesystem.toURI`), not this module's.
 */
export function resolveLocator(rootPath: string, locator: string): string | null {
  const verdict = checkLocator(locator)
  if (!verdict.safe) {
    return null
  }
  const root = usableRoot(rootPath)
  if (root === null) {
    return null
  }
  return `${root}/${verdict.segments.join('/')}`
}

/** Per-subtitle file names to look for beside a video, without their extension. */
export function subtitleStem(locator: string): string | null {
  const verdict = checkLocator(locator)
  if (!verdict.safe) {
    return null
  }
  const name = verdict.segments[verdict.segments.length - 1]
  if (name === undefined) {
    return null
  }
  const dot = name.lastIndexOf('.')
  return dot > 0 ? name.slice(0, dot) : name
}
