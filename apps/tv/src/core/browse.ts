/**
 * What the browse screen decides, as pure functions.
 *
 * Every rule here has a counterpart on the desktop, because the two devices show the
 * same library and an owner who learns one should not have to relearn the other:
 *
 * * paging sized to a whole number of rows, so a page never ends mid-row;
 * * filtering by kind and by text, matching title and filename;
 * * Continue Watching ordered by when playback last changed, excluding finished and
 *   hidden work — the same predicate as `Library::continue_watching` in the Rust
 *   core (`position_ms > 0 AND watched = 0 AND hidden = 0`, source not removed);
 * * an empty screen that names the situation it is actually in. "No media yet",
 *   "this folder could not be read", and "your search is too narrow" are three
 *   different situations with three different remedies, and one generic message
 *   would be a dead end.
 */

import type { LibraryItem, MediaKind, ResumeEntry } from './contract.ts'

/** Cells across a 1920-wide grid at 10-foot distance. Fewer and larger, not denser. */
export const COLUMNS = 5

/**
 * Rows drawn at once.
 *
 * Two, not three. Three rows of five only fit if the cards are shrunk until the title
 * no longer fits inside its own card; `design/cards.ts` sizes the grid to the band
 * between the header and the footer, and at this type scale that band holds two rows of
 * five readable cards. A third row was drawn below the bottom of the set.
 *
 * This is how many rows are *visible*, not how many the library has: the grid is a
 * window over the whole list (`core/nav.ts`), so a thousand files scroll rather than
 * paging.
 */
export const ROWS = 2

/** The kind filter the screen offers. */
export type KindFilter = 'all' | 'movies' | 'series'

/** The kind a filter maps onto, or `null` for every kind. */
export function kindOf(filter: KindFilter): MediaKind | null {
  switch (filter) {
    case 'all':
      return null
    case 'movies':
      return 'movie'
    case 'series':
      return 'series'
  }
}

/** A filter as the user set it. */
export interface BrowseFilter {
  readonly kind: KindFilter
  readonly text: string
}

/** The filter a screen starts with. */
export const NO_FILTER: BrowseFilter = { kind: 'all', text: '' }

/** Case- and accent-insensitive enough for a remote: lowercase, trimmed. */
function normalise(text: string): string {
  return text.trim().toLowerCase()
}

/**
 * The order a library is shown in.
 *
 * Title first, then season and episode, then the stored path as a tie-break so the
 * order is stable between renders. Sorting in the screen would have to be repeated in
 * every screen that shows a list; sorting here means the rail's three library screens
 * cannot disagree about what "alphabetical" means.
 *
 * The comparison is deliberately not `localeCompare`: a 2021 TV's collation differs by
 * firmware, and a library whose order changed between two sets is a bug the owner
 * cannot describe.
 */
export function sortForDisplay(items: readonly LibraryItem[]): LibraryItem[] {
  const key = (item: LibraryItem): string => item.title.toLowerCase()
  return [...items].sort((left, right) => {
    const byTitle = key(left) < key(right) ? -1 : key(left) > key(right) ? 1 : 0
    if (byTitle !== 0) {
      return byTitle
    }
    const bySeason = (left.season ?? 0) - (right.season ?? 0)
    if (bySeason !== 0) {
      return bySeason
    }
    const byEpisode = (left.episode ?? 0) - (right.episode ?? 0)
    if (byEpisode !== 0) {
      return byEpisode
    }
    return left.relativePath < right.relativePath ? -1 : left.relativePath > right.relativePath ? 1 : 0
  })
}

/**
 * The items one rail destination shows.
 *
 * `movies` includes files the filename rule could not read (`kind: 'other'`) on
 * purpose. A container this app could not name is still a film the owner put on the
 * drive, and a filter that hid it would make the file unreachable from every screen but
 * Home — which is the defect this whole change exists to remove, in miniature.
 */
export function itemsFor(
  items: readonly LibraryItem[],
  kind: KindFilter,
): LibraryItem[] {
  switch (kind) {
    case 'all':
      return sortForDisplay(items)
    case 'movies':
      return sortForDisplay(items.filter((item) => item.kind !== 'series'))
    case 'series':
      return sortForDisplay(items.filter((item) => item.kind === 'series'))
  }
}

/**
 * Applies a filter to the catalog.
 *
 * The text matches the title or the filename, because an owner searching for
 * "S02E04" is thinking about the file and an owner searching for a film name is
 * thinking about the title, and neither of them knows which one this app stores in
 * which field.
 */
export function filterItems(
  items: readonly LibraryItem[],
  filter: BrowseFilter,
): readonly LibraryItem[] {
  const wanted = kindOf(filter.kind)
  const needle = normalise(filter.text)
  return items.filter((item) => {
    if (wanted !== null && item.kind !== wanted) {
      return false
    }
    if (needle.length === 0) {
      return true
    }
    return (
      normalise(item.title).includes(needle) || normalise(item.relativePath).includes(needle)
    )
  })
}

/**
 * A situation an empty screen is actually in.
 *
 * Each one carries its own copy because each has a different remedy; a single
 * "nothing here" would tell the owner nothing about what to do.
 */
export type EmptySituation =
  | 'first_run'
  | 'scanning'
  | 'source_unreadable'
  | 'filtered'
  | 'empty_library'

/** The title and detail an empty screen shows. */
export interface EmptyState {
  readonly situation: EmptySituation
  readonly title: string
  readonly detail: string
}

/** What the screen knows when it has nothing to show. */
export interface EmptyInput {
  /** How many sources are granted, including ones that are unavailable. */
  readonly sourceCount: number
  /** How many items the catalog holds before filtering. */
  readonly catalogCount: number
  /** True while a scan is running. */
  readonly scanning: boolean
  /** How many granted sources are not available right now. */
  readonly unavailableSources: number
  /** The text the user has typed into the filter. */
  readonly filterText: string
}

/**
 * The empty screen to show, or `null` when there is something to show.
 *
 * Order matters, and it is the order of what the owner can act on: a running scan
 * explains itself first, then a folder that could not be read (the remedy is in
 * Settings), then a filter that is too narrow (the remedy is to clear it), and only
 * then the two situations where there is genuinely nothing yet.
 */
export function emptyState(input: EmptyInput): EmptyState | null {
  if (input.unavailableSources > 0) {
    return {
      situation: 'source_unreadable',
      title: 'A drive is not readable right now.',
      detail: 'Plug it back in, then Scan for media in Settings. Nothing in your library was lost.',
    }
  }
  if (input.scanning) {
    return {
      situation: 'scanning',
      title: 'Looking for media…',
      detail: 'Files appear as they are found. You can keep browsing while this runs.',
    }
  }
  if (input.filterText.trim().length > 0) {
    return {
      situation: 'filtered',
      title: 'Nothing matches that search.',
      detail: 'Clear the search, or switch the filter back to All.',
    }
  }
  if (input.sourceCount === 0) {
    return {
      situation: 'first_run',
      title: 'No media yet.',
      // A television has no folder picker: the remedy is a drive and a scan, in that
      // order, and saying so is the difference between a dead end and a next step.
      detail: 'Plug a drive with films or episodes into the TV, then choose Media Sources in the menu and run Scan for media.',
    }
  }
  if (input.catalogCount === 0) {
    return {
      situation: 'empty_library',
      title: 'This drive has no media in it.',
      detail: 'Put films or episodes on the drive, then scan again from Media Sources.',
    }
  }
  return null
}

/**
 * Continue Watching, most recently touched first.
 *
 * The predicate is the core's: something with a position, not finished, not hidden,
 * from a source that has not been removed. Ordering is by `updatedMs` descending,
 * with the item id as a tie-break so two entries touched in the same millisecond
 * keep a stable order between renders rather than swapping places.
 */
export function continueWatching(
  items: readonly LibraryItem[],
  limit: number,
): readonly ResumeEntry[] {
  const bounded = Math.max(1, Math.trunc(limit))
  return items
    .filter(
      (item) =>
        item.positionMs > 0 &&
        !item.watched &&
        !item.hidden &&
        item.available &&
        item.sourceId !== '',
    )
    .sort((left, right) => {
      if (right.updatedMs !== left.updatedMs) {
        return right.updatedMs - left.updatedMs
      }
      return left.id < right.id ? -1 : left.id > right.id ? 1 : 0
    })
    .slice(0, bounded)
    .map((item) => ({ item, positionMs: item.positionMs, durationMs: item.durationMs }))
}

/** What Home shows, and why. */
export interface HomeShelf {
  readonly heading: string
  /** The line under the heading: a count, never a claim. */
  readonly note: string
  readonly items: readonly LibraryItem[]
  /** True when the shelf is resume points rather than the whole library. */
  readonly fromResume: boolean
}

/**
 * What the Home screen draws.
 *
 * This is the fix for the defect the owner reported on this set: the scan catalogued
 * over a thousand files and Home drew nothing, because Home only ever drew *Continue
 * watching* and nothing had been played yet. A library is not only what you were in
 * the middle of.
 *
 * So the rule is: resume points when there are any, and **the whole library** when there
 * are not. The second half is the important one — a fresh install, a new drive, or a
 * viewer who finished everything still opens onto their media, with the rail beside it
 * for Movies, TV Shows and Search.
 */
export function homeShelf(items: readonly LibraryItem[]): HomeShelf {
  const resume = continueWatching(items, Number.MAX_SAFE_INTEGER)
  const total = `${items.length} ${items.length === 1 ? 'file' : 'files'} in the library`
  if (resume.length > 0) {
    return {
      heading: 'Continue watching',
      note: `${resume.length} in progress · ${total}`,
      items: resume.map((entry) => entry.item),
      fromResume: true,
    }
  }
  return { heading: 'Your library', note: total, items: sortForDisplay(items), fromResume: false }
}

/**
 * A position as a remote-sized label.
 *
 * Hours are dropped when there are none, because "12:04" is read at a glance and
 * "0:12:04" is read twice.
 */
export function formatPosition(positionMs: number): string {
  const total = Math.max(0, Math.trunc(positionMs / 1000))
  const hours = Math.trunc(total / 3600)
  const minutes = Math.trunc((total % 3600) / 60)
  const seconds = total % 60
  const two = (value: number): string => value.toString().padStart(2, '0')
  return hours > 0 ? `${hours}:${two(minutes)}:${two(seconds)}` : `${minutes}:${two(seconds)}`
}

/** How far through a file a resume point is, as a fraction in `0..1`. */
export function progressFraction(positionMs: number, durationMs: number): number {
  if (durationMs <= 0) {
    return 0
  }
  const fraction = positionMs / durationMs
  if (!Number.isFinite(fraction)) {
    return 0
  }
  return Math.min(1, Math.max(0, fraction))
}
