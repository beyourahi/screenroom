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
 * Rows per page.
 *
 * Two, not three. A page of five-by-three only fits if the cards are shrunk until the
 * 42-pixel title no longer fits inside its own card; `design/cards.ts` sizes the grid
 * to the band between the header and the footer, and at this type scale that band holds
 * two rows of five readable cards. A third row was drawn below the bottom of the set.
 */
export const ROWS = 2

/** Items per page, always a whole number of rows. */
export const PAGE_SIZE = COLUMNS * ROWS

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

/** How many pages a result set has, never fewer than one. */
export function pageCount(count: number): number {
  if (count <= 0) {
    return 1
  }
  return Math.ceil(count / PAGE_SIZE)
}

/** The page a requested number resolves to, given how many there are. */
export function clampPage(page: number, count: number): number {
  const last = pageCount(count) - 1
  if (!Number.isFinite(page)) {
    return 0
  }
  const whole = Math.trunc(page)
  if (whole < 0) {
    return 0
  }
  return whole > last ? last : whole
}

/** The slice of items shown on one page. */
export function pageItems(
  items: readonly LibraryItem[],
  page: number,
): readonly LibraryItem[] {
  const start = clampPage(page, items.length) * PAGE_SIZE
  return items.slice(start, start + PAGE_SIZE)
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
      detail: 'Plug a drive with films or episodes into the TV, then run Scan for media in Settings (press Info).',
    }
  }
  if (input.catalogCount === 0) {
    return {
      situation: 'empty_library',
      title: 'This drive has no media in it.',
      detail: 'Put films or episodes on the drive, then Scan for media in Settings.',
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
