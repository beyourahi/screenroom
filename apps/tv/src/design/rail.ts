/**
 * The navigation rail, drawn once and shared by every screen.
 *
 * The rail is a persistent column of destinations down the left edge: Home, Movies, TV
 * Shows, Search, Media Sources, Settings. It is always on screen while the viewer is
 * browsing, which is what makes every feature reachable with the D-pad alone. Before
 * it existed, the same routes were registered and nothing navigated to them — Movies,
 * TV Shows and the library grid were literally unreachable, and Settings was behind
 * the remote's Info key.
 *
 * Two things are deliberate:
 *
 * * **The rows are decided here, the drawing is one component.** `railRows` is a pure
 *   function with tests; `src/components/Rail.ts` draws whatever it is handed. The
 *   screens own only which destination is active and where the cursor is.
 * * **No focus ring.** The focused row is a filled surface with inverted ink, the same
 *   treatment a focused card gets (`design/cards.ts`). A ring drawn beside a fill is a
 *   second visual language for the same fact.
 *
 * The geometry is shared with the grid: `RAIL_WIDTH` is how the screens know where the
 * content may start, so the menu and the cards cannot disagree about the boundary.
 */

import { hex, TOKENS, type Tokens } from './tokens.ts'
import { RAIL_ENTRIES } from '../core/nav.ts'

/** How much of the screen the rail occupies. The grid starts after it. */
export const RAIL_WIDTH = 320

/** One drawn rail row, with every value the template needs already decided. */
export interface RailRow {
  readonly key: string
  readonly label: string
  readonly route: string
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
  /** Where the label starts, measured from the row's own left edge. */
  readonly labelX: number
  readonly labelY: number
  readonly fill: string
  readonly labelColor: string
  /** The 6-pixel bar that marks the destination the viewer is already on. */
  readonly markerColor: string
  /** True for the destination this route belongs to. */
  readonly active: boolean
  /** True for the row the remote is currently on. */
  readonly focused: boolean
}

/** The geometry every screen's rail uses. Derived from the token scale, not guessed. */
export interface RailLayout {
  readonly pad: number
  readonly rowX: number
  readonly rowWidth: number
  readonly rowHeight: number
  readonly rowGap: number
  /** Where the first row starts, below the app's own name. */
  readonly top: number
  readonly labelX: number
  readonly labelY: number
  readonly titleSize: number
  readonly labelSize: number
  readonly markerWidth: number
}

/** The rail's geometry, at this token scale. */
export function railLayout(tokens: Tokens = TOKENS): RailLayout {
  const pad = Math.round(tokens.spacing.shellPad)
  const rowHeight = Math.round(tokens.control.lg * 1.2)
  const gap = Math.round(tokens.type.caption * 0.9)
  const labelSize = Math.round(tokens.type.body)
  const titleSize = Math.round(tokens.type.bodyLg)
  const markerWidth = 6
  const rowX = Math.round(pad * 0.4)
  return {
    pad,
    rowX,
    rowWidth: RAIL_WIDTH - rowX * 2,
    rowHeight,
    rowGap: rowHeight + gap,
    // The app's own name sits above the first row, sized from the same title scale as a
    // screen heading so the rail reads as part of the shell rather than as a list.
    top: Math.round(pad + tokens.type.title * 2.3),
    labelX: markerWidth + Math.round(tokens.type.caption * 0.7),
    labelY: Math.round((rowHeight - labelSize * 1.25) / 2),
    titleSize,
    labelSize,
    markerWidth,
  }
}

/**
 * The rail rows for a route and a cursor.
 *
 * `activeIndex` is the destination the route belongs to (-1 for a screen with no rail),
 * and `focusedIndex` is where the remote is — which is only meaningful while the rail
 * owns the cursor. Keeping them separate is what lets the rail show "you are here" and
 * "the remote is here" at the same time.
 */
export function railRows(
  activeIndex: number,
  focusedIndex: number,
  focused: boolean,
  tokens: Tokens = TOKENS,
): RailRow[] {
  const layout = railLayout(tokens)
  return RAIL_ENTRIES.map((entry, index) => {
    const isFocused = focused && index === focusedIndex
    const active = index === activeIndex
    return {
      key: entry.key,
      label: entry.label,
      route: entry.route,
      x: layout.rowX,
      y: layout.top + index * layout.rowGap,
      width: layout.rowWidth,
      height: layout.rowHeight,
      labelX: layout.labelX,
      labelY: layout.labelY,
      fill: isFocused
        ? hex(tokens.palette.focusSurface)
        : active
          ? hex(tokens.palette.surface2)
          : hex(tokens.palette.surface1),
      labelColor: isFocused
        ? hex(tokens.palette.focusForeground)
        : active
          ? hex(tokens.palette.foreground)
          : hex(tokens.palette.inkMuted),
      markerColor: hex(tokens.palette.signal),
      active,
      focused: isFocused,
    }
  })
}


