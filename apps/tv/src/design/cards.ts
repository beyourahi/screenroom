/**
 * What a card looks like, and where the cards go.
 *
 * This module exists for two reasons, and both of them are about not putting decisions
 * in a template.
 *
 * 1. **A template is not a good place for a rule.** Blits templates are compiled to
 *    property assignments, so an expression like `$i === $focusIndex ? colour : other`
 *    is evaluated per node per frame and cannot be unit tested at all. Deciding the
 *    card's fill, scale and label here means the whole visual contract is a pure
 *    function with tests, and the template only copies values out.
 * 2. **The grid geometry has to follow the tokens.** Card size, gutters and the shell
 *    padding all derive from the same token set the desktop uses, so a change to the
 *    scale moves both platforms. A hard-coded 340 in a template would drift from the
 *    design system the moment the scale changed — on a canvas nobody can diff.
 *
 * Nothing here is TV-specific except the numbers it is given.
 */

import { FOCUS_SCALE, hex, TOKENS, type Tokens } from './tokens.ts'
import { formatPosition, progressFraction } from '../core/browse.ts'
import type { LibraryItem } from '../core/contract.ts'

/** The authored surface. The TV scales the whole canvas; this is the drawing space. */
export const APP_WIDTH = 1920
export const APP_HEIGHT = 1080

/** Where the cards sit and how big they are, in app pixels. */
export interface GridGeometry {
  readonly columns: number
  readonly rows: number
  readonly originX: number
  /** Where the first row starts: the vertical bound the caller asked for. */
  readonly originY: number
  readonly cardWidth: number
  readonly cardHeight: number
  readonly gapX: number
  readonly gapY: number
  /** How wide the video/art area inside a card is. */
  readonly artWidth: number
  readonly artHeight: number
  /** Where the title line starts, measured from the card's own top. */
  readonly titleY: number
  /** Where the sub-title line starts, measured from the card's own top. */
  readonly subtitleY: number
}

/** The vertical band a grid has to fit into, in app pixels. */
export interface GridBounds {
  /** The first pixel the grid may use; the screen's header ends here. */
  readonly top: number
  /** The last pixel the grid may use; the footer starts here. */
  readonly bottom: number
}

/**
 * The grid, derived from the token scale rather than from literal pixels.
 *
 * The focused card is grown by `FOCUS_SCALE`, and the gutter is sized to hold that
 * growth: otherwise the focused card would overlap its neighbour and, on a TV, cover
 * the very title the user is reading. `gapX` is therefore `cardWidth *
 * (FOCUS_SCALE - 1)` plus the design system's own section spacing.
 */
export function gridFor(
  columns: number,
  rows: number,
  tokens: Tokens = TOKENS,
  width: number = APP_WIDTH,
  bounds: GridBounds = {
    top: Math.round(TOKENS.spacing.shellPad),
    bottom: APP_HEIGHT - Math.round(TOKENS.spacing.shellPad),
  },
): GridGeometry {
  // Every count is sanitised, not merely floored: `Math.max(1, Math.trunc(NaN))` is
  // `NaN`, which would propagate into every card's width and place the whole grid at
  // `NaN` — a blank screen with nothing to report. An unmeasured layout must produce a
  // usable grid instead.
  const safeColumns = countOrOne(columns)
  const safeRows = countOrOne(rows)
  const safeWidth = Number.isFinite(width) && width > 0 ? width : APP_WIDTH
  const originX = Math.round(tokens.spacing.shellPad)
  const originY = Number.isFinite(bounds.top) ? Math.round(bounds.top) : 0
  const bottom = Number.isFinite(bounds.bottom) ? Math.round(bounds.bottom) : APP_HEIGHT

  // The two lines the card has to hold, and therefore the strip beneath the art. It is
  // derived from the type scale rather than guessed: a strip sized independently of the
  // text inside it clips the title, which is what the first TV build did — the title and
  // its subtitle were drawn past the card's own bottom edge.
  const titleGap = Math.round(tokens.type.caption * 1.2)
  const titleLine = Math.round(tokens.type.bodyLg * 1.5)
  const subtitleLine = Math.round(tokens.type.caption * 1.6)
  const titleStrip = titleGap + titleLine + subtitleLine

  const usable = safeWidth - originX * 2
  const sectionGap = Math.round(tokens.spacing.section / 2)
  const gapX = Math.max(sectionGap, Math.round(tokens.type.body * 1.5))
  const gapY = gapX

  // A grid has to fit in *both* directions. Sizing from the width alone is how a third
  // row ends up drawn below the television's bottom edge; the height bound is what the
  // caller's footer implies.
  const byWidth = Math.floor((usable - gapX * (safeColumns - 1)) / safeColumns)
  const usableHeight = Math.max(1, bottom - originY)
  const rowSpace = (usableHeight - gapY * (safeRows - 1)) / safeRows
  const byHeight = Math.floor((rowSpace - titleStrip) * (16 / 9))
  const cardWidth = Math.max(1, Math.min(byWidth, byHeight))

  const artWidth = cardWidth
  const artHeight = Math.round((artWidth * 9) / 16)
  const cardHeight = artHeight + titleStrip

  return {
    columns: safeColumns,
    rows: safeRows,
    originX,
    originY,
    cardWidth,
    cardHeight,
    gapX,
    gapY,
    artWidth,
    artHeight,
    titleY: artHeight + titleGap,
    subtitleY: artHeight + titleGap + titleLine,
  }
}

/** A whole count of at least one, from a value that may be absent or not a number. */
function countOrOne(value: number): number {
  if (!Number.isFinite(value)) {
    return 1
  }
  const whole = Math.trunc(value)
  return whole >= 1 ? whole : 1
}

/** Where one cell sits, by index. */
export function cellPosition(
  index: number,
  geometry: GridGeometry,
): { readonly x: number; readonly y: number } {
  const safe = Math.max(0, Math.trunc(index))
  const column = safe % geometry.columns
  const row = Math.floor(safe / geometry.columns)
  return {
    x: geometry.originX + column * (geometry.cardWidth + geometry.gapX),
    y: geometry.originY + row * (geometry.cardHeight + geometry.gapY),
  }
}

/**
 * One card, with every value the template needs already decided.
 *
 * Colours are `#rrggbb` strings because that is what the renderer takes from a
 * template; the alpha channel is dropped deliberately, since the tokens that carry one
 * (`hair`) are not used as a fill.
 */
export interface Card {
  readonly id: string
  readonly title: string
  readonly subtitle: string
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
  readonly fill: string
  readonly titleColor: string
  readonly subtitleColor: string
  /** The scale the renderer should draw this card at. 1 unless focused. */
  readonly scale: number
  /** True for the card the remote is on. Drives the label and the progress bar. */
  readonly focused: boolean
  /** How far through the file this item is, 0..1. */
  readonly progress: number
  /** The same fraction as a pixel width, so the template copies rather than computes. */
  readonly progressWidth: number
  /** False when there is nothing to show, so the bar is hidden rather than empty. */
  readonly progressVisible: boolean
  readonly progressColor: string
  /** True when the item is not playable right now, which the card must show. */
  readonly unavailable: boolean
}

/** The parts of an item a card needs. Keeps the function usable by any screen. */
export type CardItem = LibraryItem

/**
 * Builds the cards for one page.
 *
 * The subtitle is the one place the app tells the owner something they cannot see: the
 * episode number for a series, the year for a film, and — when the file cannot be
 * played right now — that fact, in place of the year. A card whose source is gone is
 * drawn muted and says so rather than being silently hidden, because the owner's
 * progress on it is still theirs (`docs/ADR-003`).
 */
export function buildCards(
  items: readonly CardItem[],
  focusIndex: number,
  geometry: GridGeometry,
  tokens: Tokens = TOKENS,
): Card[] {
  return items.map((item, index) => {
    const position = cellPosition(index, geometry)
    const focused = index === focusIndex
    const progress = progressFraction(item.positionMs, item.durationMs)
    return {
      id: item.id,
      title: item.title,
      subtitle: subtitleFor(item),
      x: position.x,
      y: position.y,
      width: geometry.cardWidth,
      height: geometry.cardHeight,
      fill: focused ? hex(tokens.palette.focusSurface) : hex(tokens.palette.surface1),
      titleColor: focused
        ? hex(tokens.palette.focusForeground)
        : hex(tokens.palette.foreground),
      subtitleColor: focused
        ? hex(tokens.palette.focusForeground)
        : hex(tokens.palette.inkMuted),
      scale: focused ? FOCUS_SCALE : 1,
      focused,
      progress,
      progressWidth: Math.round(geometry.artWidth * progress),
      progressVisible: progress > 0 && progress < 1,
      progressColor: focused ? hex(tokens.palette.focusForeground) : hex(tokens.palette.signal),
      unavailable: !item.available,
    }
  })
}

/** The one-line description under a card's title. */
export function subtitleFor(item: CardItem): string {
  if (!item.available) {
    return 'Not readable — reconnect the drive'
  }
  const parts: string[] = []
  if (item.kind === 'series' && item.season !== null && item.episode !== null) {
    parts.push(`S${pad(item.season)}E${pad(item.episode)}`)
  } else if (item.year !== null) {
    parts.push(String(item.year))
  }
  if (item.watched) {
    parts.push('Watched')
  } else if (item.positionMs > 0) {
    const remaining =
      item.durationMs > item.positionMs ? item.durationMs - item.positionMs : 0
    parts.push(
      item.durationMs > 0 ? `${formatPosition(remaining)} left` : formatPosition(item.positionMs),
    )
  }
  return parts.length > 0 ? parts.join(' · ') : 'Not started'
}

/** Two-digit episode and season numbers, which is how a series is talked about. */
function pad(value: number): string {
  return Math.max(0, Math.trunc(value)).toString().padStart(2, '0')
}
