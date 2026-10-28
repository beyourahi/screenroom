/**
 * The shell's navigation, as arithmetic.
 *
 * Every screen in this app shares one persistent rail on the left, and the rail is the
 * only way a remote-only viewer can reach anything. Before this module existed the
 * routes for Movies, TV Shows and the library grid were registered and *nothing
 * navigated to them*: the only working path off Home was the remote's Info key, which
 * put the whole library behind a button most remotes do not label. A television app
 * whose navigation depends on a model-specific key is an app with no navigation.
 *
 * Two rules live here, and both are decisions rather than drawing:
 *
 * 1. **Which destinations exist, and which one a route belongs to.** The rail is a
 *    fixed list, so a new screen cannot appear in it without an entry here.
 * 2. **How a press moves between the rail and the content.** Two zones, one cursor in
 *    each, and a rule for every direction that never invents focus: left from the
 *    first column of the grid goes to the rail, right from the rail returns to the
 *    card the viewer was on, and nothing wraps.
 *
 * The grid itself is a *window* over the whole list rather than a page. A drive with a
 * thousand files is one long list; paging it meant a press count the owner could not
 * predict, and a page that ended mid-row. `gridWindow` keeps the cursor inside a band
 * of whole rows and slides that band, which is what makes Down mean "next row" for
 * every file on the drive.
 */

import { clampIndex, moveFocus, type Direction } from './focus.ts'

/** One destination in the rail. */
export interface RailEntry {
  /** Stable key, also the route's own name. */
  readonly key: string
  /** What the rail draws. Short, because it is read at three metres. */
  readonly label: string
  /** The route pressing Enter opens. */
  readonly route: string
}

/**
 * The rail, in the order it is drawn.
 *
 * Home is first because it is where the app opens and therefore where the cursor
 * starts. Settings is last because it is the least-visited, and Media Sources sits
 * beside it because that is where the scan lives.
 */
export const RAIL_ENTRIES: readonly RailEntry[] = [
  { key: 'home', label: 'Home', route: '/' },
  { key: 'movies', label: 'Movies', route: '/movies' },
  { key: 'series', label: 'TV Shows', route: '/series' },
  { key: 'search', label: 'Search', route: '/search' },
  { key: 'sources', label: 'Media Sources', route: '/sources' },
  { key: 'settings', label: 'Settings', route: '/settings' },
]

/** The index of the entry a route belongs to, or -1 for a screen with no rail. */
export function railIndexFor(route: string): number {
  // A route may carry a query or a trailing slash; neither changes the destination.
  const trimmed = route.split('?')[0]?.replace(/\/+$/, '') ?? ''
  const path = trimmed.length === 0 ? '/' : trimmed
  return RAIL_ENTRIES.findIndex((entry) => entry.route === path)
}

/** The entry for a route, or `null`. */
export function railEntryFor(route: string): RailEntry | null {
  const index = railIndexFor(route)
  return index < 0 ? null : (RAIL_ENTRIES[index] ?? null)
}

/** Which of the two focus zones a press is currently aimed at. */
export type NavZone = 'rail' | 'content'

/** Where the remote's cursor is: a zone, and one index inside each zone. */
export interface NavPosition {
  readonly zone: NavZone
  readonly rail: number
  readonly content: number
}

/** What the cursor is allowed to move over. */
export interface NavBounds {
  readonly railCount: number
  readonly contentCount: number
  /** The column count the grid is actually drawn with. */
  readonly columns: number
}

/** The cursor at the start of a screen, before anything has been remembered. */
export function initialNav(zone: NavZone = 'content'): NavPosition {
  return { zone, rail: 0, content: 0 }
}

/** A whole count of at least zero, from a value that may be absent or not a number. */
function countOf(value: number): number {
  return Number.isFinite(value) && value > 0 ? Math.trunc(value) : 0
}

/** A column count of at least one, so a bad layout degrades to a list rather than NaN. */
function columnsOf(value: number): number {
  return Number.isFinite(value) && value >= 1 ? Math.trunc(value) : 1
}

/**
 * The position one press moves to.
 *
 * The content zone uses the same flat-index rules as the desktop (`focus.ts`), so an
 * arrow key behaves the same on the sofa as at the desk. The rail is the one addition,
 * and its rules are deliberately narrow:
 *
 * * up and down move inside the rail and stop at both ends;
 * * right enters the content at whatever index that zone already held, so coming back
 *   from the rail lands on the card the viewer left rather than on the first one;
 * * left on the grid's first column returns to the rail, and left anywhere else is an
 *   ordinary one-card move.
 */
export function moveNav(
  position: NavPosition,
  direction: Direction,
  bounds: NavBounds,
): NavPosition {
  const railCount = Math.max(1, countOf(bounds.railCount) || RAIL_ENTRIES.length)
  const contentCount = countOf(bounds.contentCount)
  const columns = columnsOf(bounds.columns)

  const rail = Math.max(0, clampIndex(position.rail, railCount))
  const content = contentCount === 0 ? -1 : clampIndex(position.content, contentCount)
  // With nothing to browse the rail is the only zone, whatever the screen asked for:
  // a cursor in an empty grid is a cursor nobody can see.
  const zone: NavZone = contentCount === 0 ? 'rail' : position.zone

  if (zone === 'rail') {
    switch (direction) {
      case 'up':
        return { zone, rail: Math.max(0, rail - 1), content }
      case 'down':
        return { zone, rail: Math.min(railCount - 1, rail + 1), content }
      case 'right':
        // With nothing to browse there is nowhere to move to, and a cursor placed in an
        // empty grid is a cursor nobody can see.
        return contentCount === 0
          ? { zone, rail, content }
          : { zone: 'content', rail, content: Math.max(0, content) }
      case 'left':
        return { zone, rail, content }
    }
  }

  if (direction === 'left' && content % columns === 0) {
    return { zone: 'rail', rail, content }
  }
  return { zone: 'content', rail, content: moveFocus(content, contentCount, columns, direction) }
}

/**
 * The band of rows the grid draws, and where it starts.
 *
 * `first` is an absolute index into the whole list, so a card knows which file it is
 * even though only ten of them are on screen. The band always begins at a row
 * boundary, which is why `first` is a multiple of the column count: a window that
 * started mid-row would draw the grid skewed by one card.
 *
 * The cursor is kept in the *last* visible row once the list is long enough, so the
 * first press of Down after a full screen scrolls the list rather than moving a cursor
 * inside a page. When there is less than a screen left the window stops at the end and
 * the cursor walks down inside it, which is what stops the last row from being
 * unreachable.
 */
export interface GridWindow {
  /** Absolute index of the first drawn card. */
  readonly first: number
  /** How many cards are drawn. */
  readonly count: number
  /** How many rows are drawn. */
  readonly rows: number
  /** How many rows have been scrolled past. */
  readonly scrollRows: number
  /** How many rows the whole list has. */
  readonly totalRows: number
}

/** The window over a list, given the cursor's absolute index. */
export function gridWindow(
  total: number,
  columns: number,
  visibleRows: number,
  focusIndex: number,
): GridWindow {
  const safeTotal = countOf(total)
  const cols = columnsOf(columns)
  const rowsVisible = Math.max(1, countOf(visibleRows) || 1)
  if (safeTotal === 0) {
    return { first: 0, count: 0, rows: 0, scrollRows: 0, totalRows: 0 }
  }
  const totalRows = Math.ceil(safeTotal / cols)
  const focus = clampIndex(focusIndex, safeTotal)
  const focusRow = Math.min(totalRows - 1, Math.max(0, Math.floor(focus / cols)))
  const lastStart = Math.max(0, totalRows - rowsVisible)
  const scrollRows = Math.min(lastStart, Math.max(0, focusRow - rowsVisible + 1))
  const first = scrollRows * cols
  const count = Math.min(safeTotal - first, rowsVisible * cols)
  return {
    first,
    count,
    rows: Math.min(rowsVisible, count === 0 ? 0 : Math.ceil(count / cols)),
    scrollRows,
    totalRows,
  }
}

/** A remembered cursor, per route. */
export interface NavMemory {
  get(key: string): NavPosition | undefined
  /** Remembers a position, clamped to the bounds that were true when it was set. */
  set(key: string, position: NavPosition, bounds: NavBounds): void
  clear(key: string): void
}

/** An in-memory navigation memory. One per app run, held by the runtime. */
export function createNavMemory(): NavMemory {
  const remembered = new Map<string, NavPosition>()
  return {
    get(key: string): NavPosition | undefined {
      return remembered.get(key)
    },
    set(key: string, position: NavPosition, bounds: NavBounds): void {
      const railCount = Math.max(1, countOf(bounds.railCount) || RAIL_ENTRIES.length)
      const contentCount = countOf(bounds.contentCount)
      remembered.set(key, {
        zone: contentCount === 0 ? 'rail' : position.zone,
        rail: Math.max(0, clampIndex(position.rail, railCount)),
        content: contentCount === 0 ? -1 : clampIndex(position.content, contentCount),
      })
    },
    clear(key: string): void {
      remembered.delete(key)
    },
  }
}

/**
 * The cursor a screen should open with.
 *
 * A remembered position is restored when it still fits; a list that shrank while the
 * viewer was elsewhere (a rescan removing files) lands on its last card rather than on
 * nothing. A screen opened for the first time starts where the caller says, which is
 * the content for a library and the rail for Home — Home opens on the rail because
 * that is the one place the navigation is guaranteed to be found.
 */
export function restoreNav(
  memory: NavMemory,
  key: string,
  bounds: NavBounds,
  fallback: NavZone,
): NavPosition {
  const remembered = memory.get(key)
  if (remembered === undefined) {
    const base = initialNav(fallback)
    return contentOf(bounds) === 0 ? { ...base, zone: 'rail', content: -1 } : base
  }
  const contentCount = contentOf(bounds)
  const railCount = Math.max(1, countOf(bounds.railCount) || RAIL_ENTRIES.length)
  if (contentCount === 0) {
    return {
      zone: 'rail',
      rail: Math.max(0, clampIndex(remembered.rail, railCount)),
      content: -1,
    }
  }
  return {
    zone: remembered.zone,
    rail: Math.max(0, clampIndex(remembered.rail, railCount)),
    content: clampIndex(remembered.content, contentCount),
  }
}

function contentOf(bounds: NavBounds): number {
  return countOf(bounds.contentCount)
}
