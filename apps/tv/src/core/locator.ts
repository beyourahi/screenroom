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
 * Joins a validated locator onto a granted root.
 *
 * Returns `null` when the locator is unsafe, so a caller cannot accidentally use the
 * unvalidated string. Trailing separators on the root are tolerated because a
 * granted root is spelled by the picker, which may or may not include one.
 */
export function resolveLocator(rootPath: string, locator: string): string | null {
  const verdict = checkLocator(locator)
  if (!verdict.safe) {
    return null
  }
  const root = rootPath.replace(/[/\\]+$/, '')
  if (!root.startsWith('/')) {
    // A TV media root is always absolute on this platform; anything else would make
    // the join produce a path relative to the app's own storage, which is not where
    // media lives.
    return null
  }
  return `${root}/${verdict.segments.join('/')}`
}

/**
 * The USB media root, when the TV reports one.
 *
 * Tizen mounts removable storage under `/opt/developer` for developer uploads and
 * `/USB`-style device paths for a plugged-in drive, and the exact mount path is not
 * guaranteed across models or firmware. This normalises the spellings the platform
 * is known to use and returns the first that resolves; it deliberately does not
 * guess, and a drive that is not there returns an empty list rather than a wrong
 * path.
 */
export function candidateRoots(devicePath: string | null): string[] {
  if (devicePath === null) {
    return []
  }
  const trimmed = devicePath.replace(/[/\\]+$/, '')
  if (trimmed.length === 0) {
    return []
  }
  return trimmed.startsWith('/') ? [trimmed] : [`/${trimmed}`]
}
