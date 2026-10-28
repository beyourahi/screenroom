/**
 * The scan rules, as pure functions.
 *
 * The Rust core walks a folder with `std::fs`. The TV cannot: its filesystem API lists
 * one directory at a time, gives no size and no modification time, and the drive can
 * vanish mid-walk. So the *decisions* live here, where they have tests, and the
 * platform file next door only knows how to list a directory.
 *
 * Four rules are ported from `crates/screenroom-core/src/scanner.rs`, and all four are
 * about not wrecking a library when something goes wrong:
 *
 * 1. **Only a complete walk may withdraw availability.** A walk that stopped at a
 *    bound, or that hit a folder it could not read, must not make the library look
 *    empty — the owner would watch their whole collection disappear because a
 *    subfolder was busy.
 * 2. **Nothing is deleted.** A file that is gone becomes *unavailable*: its row, its
 *    position and its watch state stay, because they are the only copy of the owner's
 *    history.
 * 3. **Identity survives a rescan.** A file seen again at the same path keeps the id it
 *    was first given, so its progress stays attached to it.
 * 4. **The walk is bounded.** Entry and depth caps, reported as a truncated, incomplete
 *    scan rather than a silent one.
 */

import type { LibraryItem, Source, SourceState } from './contract.ts'
import { checkLocator } from './locator.ts'
import { extensionOf, folderHintOf, identityKeyOf, parseFilename } from './name.ts'

/**
 * Container extensions the library imports.
 *
 * Identical to the core's list. A container is not a codec promise: whether AVPlay can
 * actually decode a given file is the decoder's answer, never this list's.
 */
export const MEDIA_EXTENSIONS: readonly string[] = [
  'mkv',
  'mp4',
  'm4v',
  'avi',
  'mov',
  'webm',
  'mpeg',
  'mpg',
  'ts',
  'm2ts',
  'wmv',
  'flv',
  'ogv',
  '3gp',
]

/** Suffixes that mean a copy is still in progress. */
export const DOWNLOAD_SUFFIXES: readonly string[] = [
  '.part',
  '.crdownload',
  '.download',
  '.aria2',
  '.!qb',
]

/**
 * How many entries one scan may look at.
 *
 * The desktop's cap is 250,000, because a desktop can afford it and the owner may have
 * a media server's worth of files. A 2021 TV cannot: every listing is a round trip into
 * the platform's file service, and the scan shares the CPU with whatever is on screen.
 * 25,000 entries is far more than a personal library of films and episodes, and hitting
 * the cap is *reported* — the scan is then incomplete and withdraws nothing.
 */
export const MAX_ENTRIES = 25_000

/** How deep the walk goes. Deeper than this is a folder tree, not a library. */
export const MAX_DEPTH = 16

/** How many issues are kept. Beyond this the report says it truncated. */
export const MAX_ISSUES = 200

/** The longest stored relative path this client will accept, in characters. */
export const MAX_LOCATOR_CHARS = 1024

/** A file found under a source root, relative to it. */
export interface ScannedFile {
  readonly relativePath: string
}

/** What the scanner decided about one directory entry. */
export type EntryVerdict =
  /** A folder to descend into. */
  | 'directory'
  /** A folder this app must not open. */
  | 'hidden_directory'
  /** A file to catalogue. */
  | 'media'
  /** A file that is not media. */
  | 'ignored'
  /** A file whose copy has not finished. */
  | 'downloading'

/** True when the name is one that names a download in progress. */
export function isDownloadingName(name: string): boolean {
  const lower = name.toLowerCase()
  return DOWNLOAD_SUFFIXES.some((suffix) => lower.endsWith(suffix))
}

/** True when the name is a media container this library imports. */
export function isMediaName(name: string): boolean {
  const extension = extensionOf(name)
  return extension !== null && MEDIA_EXTENSIONS.includes(extension)
}

/**
 * What to do with one entry.
 *
 * A hidden *folder* is skipped, which is what keeps `.Trashes`, `.Spotlight-V100` and
 * a hundred other platform folders out of the walk. A hidden *file* with a media
 * extension is treated like any other, because the Rust scanner does the same and the
 * two clients are supposed to see the same library — a rule that differed here would
 * mean the same drive showed a different number of files on the sofa.
 *
 * The download check comes first: `Film.mkv.part` has the extension `part`, so an
 * extension test alone would quietly ignore a half-copied film instead of reporting it.
 */
export function classifyEntry(name: string, isDirectory: boolean): EntryVerdict {
  if (isDirectory) {
    return name.startsWith('.') ? 'hidden_directory' : 'directory'
  }
  if (isDownloadingName(name)) {
    return 'downloading'
  }
  return isMediaName(name) ? 'media' : 'ignored'
}

/** A storage as the platform reports it, reduced to what choosing one needs. */
export interface ReportedStorage {
  readonly label: string
  readonly type: string
  readonly mounted: boolean
}

/**
 * Whether this app reads media from a storage.
 *
 * Two documented facts decide it, and both are quoted rather than guessed:
 *
 * * `FileSystemStorageType` is `INTERNAL` or `EXTERNAL`, and the reference defines
 *   `EXTERNAL` as "removable storage, such as a USB drive or a memory card".
 * * The virtual-root table names `removable_…` as "the location for external storages",
 *   which is the spelling the platform uses for a plugged-in drive.
 *
 * Either is accepted, because firmware in this generation is known to report a drive
 * under one and not the other. A storage that is neither is left alone: the internal
 * flash and the virtual roots (`images`, `documents`, …) are not where a film lives,
 * and scanning them would walk the TV's own bookkeeping.
 *
 * The Settings screen lists *every* storage the TV reports with its type, its state
 * and whether this app chose it, so a device answer that contradicts the documentation
 * is visible in one screen rather than inferred from an empty library.
 */
export function scannableStorage(storage: ReportedStorage): boolean {
  if (!storage.mounted) {
    return false
  }
  const type = storage.type.toUpperCase()
  return type === 'EXTERNAL' || storage.label.toLowerCase().startsWith('removable')
}

/** True when the walk may still descend, given how deep it already is. */
export function mayDescend(depth: number): boolean {
  return depth < MAX_DEPTH
}

/** True when the walk has looked at as many entries as it is allowed to. */
export function withinEntryLimit(visited: number): boolean {
  return visited <= MAX_ENTRIES
}

/**
 * The relative path of a listed entry.
 *
 * The platform's callback is documented one way and described another: the Tizen 6.0
 * WebIDL says `onsuccess(DOMString[] names, Path path)` while the guide on the same
 * API says each element is "a path with the name of the file". Rather than trusting
 * either, an entry that already begins with the directory it came from is taken as a
 * path and left alone; a bare name is joined on. That way one platform quirk cannot
 * produce a path like `removable1/removable1/Movies`.
 */
export function joinEntry(directoryPath: string, entry: string): string {
  const trimmed = entry.replace(/^[/\\]+/, '')
  if (trimmed.length === 0) {
    return directoryPath
  }
  const prefix = directoryPath.length === 0 ? '' : `${directoryPath.replace(/[/\\]+$/, '')}/`
  if (prefix.length > 0 && trimmed.startsWith(prefix)) {
    return trimmed
  }
  return `${prefix}${trimmed}`
}

/** The final component of a path, which is the filename. */
export function baseName(path: string): string {
  const separator = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  return separator < 0 ? path : path.slice(separator + 1)
}

/** One entry the scan refused, with a stable code. */
export interface ScanIssue {
  /** Path relative to the source root, empty for a source-wide issue. */
  readonly relativePath: string
  /** Stable reason code, for example `still_downloading`. */
  readonly code: string
}

/** A bounded issue list, with the core's explicit truncation marker. */
export class Issues {
  private readonly entries: ScanIssue[] = []

  push(relativePath: string, code: string): void {
    if (this.entries.length < MAX_ISSUES) {
      this.entries.push({ relativePath, code })
      return
    }
    if (this.entries.length === MAX_ISSUES) {
      this.entries.push({ relativePath: '', code: 'additional_errors_omitted' })
    }
  }

  get length(): number {
    return this.entries.length
  }

  snapshot(): ScanIssue[] {
    return [...this.entries]
  }
}

/** What one source's scan produced. */
export interface CatalogPlan {
  /** Every item for this source, in path order. */
  readonly items: readonly LibraryItem[]
  /** Files catalogued or refreshed. */
  readonly imported: number
  /** Items kept but no longer readable, because a complete walk did not see them. */
  readonly withdrawn: number
  /** Entries the scan refused. */
  readonly issues: readonly ScanIssue[]
}

/** Everything planning a catalogue needs. */
export interface CatalogPlanInput {
  readonly sourceId: string
  /** Media files found under the root, relative to it. */
  readonly found: readonly ScannedFile[]
  /** The items currently catalogued for this source. */
  readonly existing: readonly LibraryItem[]
  /**
   * True when the walk saw every entry it could see. Only then may a missing file lose
   * its availability; a bounded or interrupted walk must leave the library as it was.
   */
  readonly complete: boolean
  readonly nowMs: number
  /** Generates an id with the given prefix. Passed in so this stays deterministic. */
  readonly newId: (prefix: string) => string
}

/** Compares two paths the way the sort needs: stable, and not locale-dependent. */
function comparePaths(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0
}

/**
 * Builds the catalogue for one source.
 *
 * Identity is carried forward, never recomputed: a file that was catalogued before
 * keeps the id it was given, because the id is what every stored position and watch
 * mark points at. Only a file that is genuinely new gets a new one.
 *
 * What this cannot do is recognise a *renamed* file. The desktop does that with a
 * content fingerprint (`crates/screenroom-core/src/fingerprint.rs`), which needs to
 * read the head and tail of the file; a TV directory listing does not report a size,
 * and reading a hundred megabytes off a USB drive to identify a film is not work a
 * television should be doing. So a rename costs that file its stored position, and the
 * handoff records it as a known limitation rather than pretending otherwise.
 */
export function planCatalog(input: CatalogPlanInput): CatalogPlan {
  const issues = new Issues()
  const existingByPath = new Map<string, LibraryItem>()
  for (const item of input.existing) {
    if (item.sourceId === input.sourceId) {
      existingByPath.set(item.relativePath, item)
    }
  }

  // Work ids already in this library, so a new file joins the work it belongs to
  // instead of creating a second copy of the same series. Recomputing the key from the
  // stored path is exact, because that is how the key was derived in the first place.
  const workByKey = new Map<string, string>()
  for (const item of input.existing) {
    if (item.sourceId !== input.sourceId) {
      continue
    }
    const parsed = parseFilename(baseName(item.relativePath), folderHintOf(item.relativePath))
    // The item's own id is the unique suffix, so an unreadable name can never join
    // anything — the same reason the core's unmatched key carries a fresh id.
    workByKey.set(identityKeyOf(parsed, item.sourceId, item.id), item.workId)
  }

  const seen = new Set<string>()
  const items: LibraryItem[] = []
  let imported = 0

  for (const file of [...input.found].sort((left, right) =>
    comparePaths(left.relativePath, right.relativePath),
  )) {
    const relativePath = file.relativePath
    if (seen.has(relativePath)) {
      // Two entries claiming one path would otherwise produce two rows with one id.
      continue
    }
    seen.add(relativePath)

    const locator = checkLocator(relativePath)
    if (!locator.safe) {
      issues.push(relativePath, 'unsafe_path')
      continue
    }
    if (relativePath.length > MAX_LOCATOR_CHARS) {
      issues.push(relativePath, 'invalid_name')
      continue
    }
    const name = baseName(relativePath)
    if (isDownloadingName(name)) {
      issues.push(relativePath, 'still_downloading')
      continue
    }
    if (!isMediaName(name)) {
      issues.push(relativePath, 'unsupported_container')
      continue
    }

    const parsed = parseFilename(name, folderHintOf(relativePath))
    const before = existingByPath.get(relativePath)
    if (before !== undefined) {
      imported += 1
      items.push({
        ...before,
        title: parsed.title,
        kind: parsed.kind,
        season: parsed.season,
        episode: parsed.episode,
        year: parsed.year,
        available: true,
      })
      continue
    }

    const itemId = input.newId('item')
    const key = identityKeyOf(parsed, input.sourceId, itemId)
    const workId = workByKey.get(key) ?? input.newId('work')
    workByKey.set(key, workId)
    imported += 1
    items.push({
      id: itemId,
      workId,
      title: parsed.title,
      kind: parsed.kind,
      season: parsed.season,
      episode: parsed.episode,
      year: parsed.year,
      relativePath,
      sourceId: input.sourceId,
      available: true,
      favorite: false,
      watched: false,
      hidden: false,
      positionMs: 0,
      durationMs: 0,
      updatedMs: 0,
    })
  }

  let withdrawn = 0
  if (input.complete) {
    // A path the walk did not see has gone. The row stays and the position stays; only
    // playback is withdrawn, and the screen says why.
    for (const item of input.existing) {
      if (item.sourceId !== input.sourceId || seen.has(item.relativePath)) {
        continue
      }
      if (item.available) {
        withdrawn += 1
      }
      items.push({ ...item, available: false })
    }
    items.sort((left, right) => comparePaths(left.relativePath, right.relativePath))
  } else {
    for (const item of input.existing) {
      if (item.sourceId !== input.sourceId || seen.has(item.relativePath)) {
        continue
      }
      items.push(item)
    }
    items.sort((left, right) => comparePaths(left.relativePath, right.relativePath))
  }

  return { items, imported, withdrawn, issues: issues.snapshot() }
}

/**
 * The items of the whole library, with one source's rows replaced.
 *
 * Other sources are left exactly as they were: a scan of one drive must never touch
 * another drive's rows, because that drive's state was not measured.
 */
export function replaceSourceItems(
  all: readonly LibraryItem[],
  sourceId: string,
  planned: readonly LibraryItem[],
): LibraryItem[] {
  const others = all.filter((item) => item.sourceId !== sourceId)
  return [...others, ...planned].sort((left, right) =>
    comparePaths(`${left.sourceId}/${left.relativePath}`, `${right.sourceId}/${right.relativePath}`),
  )
}

/**
 * Marks one source's rows playable or not, without touching anything else.
 *
 * Used when the platform reports a drive removed: the drive's rows become
 * unavailable and every stored position stays exactly where it was, which is the
 * requirement this whole file exists to keep.
 */
export function withSourceAvailability(
  all: readonly LibraryItem[],
  sourceId: string,
  available: boolean,
): LibraryItem[] {
  let changed = false
  const items = all.map((item) => {
    if (item.sourceId !== sourceId || item.available === available) {
      return item
    }
    changed = true
    return { ...item, available }
  })
  return changed ? items : [...all]
}

/** Turns a platform state string into this contract's state. */
export function sourceStateOf(mounted: boolean, seen: boolean): SourceState {
  if (!seen) {
    return 'missing'
  }
  return mounted ? 'available' : 'disconnected'
}

/**
 * Reconciles the granted sources with what the platform currently reports.
 *
 * A drive the TV no longer lists keeps its row and becomes `disconnected` — never
 * `removed`, which is a deliberate action and would hide the owner's progress. A drive
 * that comes back becomes `available` again with its rows still attached.
 */
export function reconcileSources(
  sources: readonly Source[],
  reported: readonly { readonly label: string; readonly mounted: boolean }[],
  resolvedPathOf: (label: string) => string,
): Source[] {
  const byLabel = new Map(reported.map((storage) => [storage.label, storage]))
  const reconciled = sources.map((source) => {
    const storage = byLabel.get(source.label)
    if (storage === undefined) {
      return { ...source, state: sourceStateOf(false, false) }
    }
    return { ...source, state: sourceStateOf(storage.mounted, true) }
  })
  const known = new Set(sources.map((source) => source.label))
  for (const storage of reported) {
    if (known.has(storage.label) || !storage.mounted) {
      continue
    }
    reconciled.push({
      id: `source-${storage.label}`,
      label: storage.label,
      rootPath: resolvedPathOf(storage.label),
      state: 'available',
      lastScanMs: 0,
    })
  }
  return reconciled
}

/** What one full scan of every drive did, as the Settings screen reports it. */
export interface ScanReport {
  /** Drives walked. */
  readonly scannedSources: number
  /** Files catalogued or refreshed. */
  readonly imported: number
  /** Items kept but marked unreadable. */
  readonly withdrawn: number
  /** Entries refused, bounded. */
  readonly issues: readonly ScanIssue[]
  /** True when every walk reached every entry it could see. */
  readonly complete: boolean
  /** True when the walk stopped at a bound or was interrupted. */
  readonly bounded: boolean
  /** When the scan finished, in whole milliseconds. */
  readonly finishedMs: number
  /** Why there was nothing to scan, when that is the case. */
  readonly reason: string | null
}

/** The report a device shows before it has ever scanned anything. */
export const NEVER_SCANNED: ScanReport = {
  scannedSources: 0,
  imported: 0,
  withdrawn: 0,
  issues: [],
  complete: false,
  bounded: false,
  finishedMs: 0,
  reason: 'No scan has run yet.',
}

/**
 * Reads a stored scan report, or says nothing has been scanned.
 *
 * A report is evidence, so a stored one that cannot be read is discarded rather than
 * half-believed: the Settings screen would rather say "no scan has run" than show a
 * file count nobody can account for.
 */
export function decodeScanReport(raw: unknown): ScanReport {
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) {
    return NEVER_SCANNED
  }
  const record = raw as Record<string, unknown>
  const whole = (value: unknown): number =>
    typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : 0
  const issues: ScanIssue[] = []
  if (Array.isArray(record['issues'])) {
    for (const entry of record['issues'].slice(0, MAX_ISSUES + 1)) {
      if (typeof entry !== 'object' || entry === null) {
        continue
      }
      const row = entry as Record<string, unknown>
      const relativePath = row['relativePath']
      const code = row['code']
      if (typeof relativePath !== 'string' || typeof code !== 'string') {
        continue
      }
      issues.push({ relativePath, code })
    }
  }
  const reason = record['reason']
  return {
    scannedSources: whole(record['scannedSources']),
    imported: whole(record['imported']),
    withdrawn: whole(record['withdrawn']),
    issues,
    complete: record['complete'] === true,
    bounded: record['bounded'] === true,
    finishedMs: whole(record['finishedMs']),
    reason: typeof reason === 'string' ? reason : null,
  }
}

/** A one-line summary of a scan, for the Settings screen and the ledger. */
export function describeScan(report: ScanReport): string {
  if (report.reason !== null) {
    return report.reason
  }
  const files = `${report.imported} ${report.imported === 1 ? 'file' : 'files'}`
  const drives = `${report.scannedSources} ${report.scannedSources === 1 ? 'drive' : 'drives'}`
  if (!report.complete) {
    return `${files} found on ${drives}; the walk did not finish, so nothing was marked unreadable.`
  }
  if (report.issues.length > 0) {
    return `${files} found on ${drives}; ${report.issues.length} ${
      report.issues.length === 1 ? 'entry' : 'entries'
    } skipped.`
  }
  return `${files} found on ${drives}, with nothing skipped.`
}
