import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  APP_HEIGHT,
  APP_WIDTH,
  buildCards,
  cellPosition,
  gridFor,
  subtitleFor,
} from '../src/design/cards.ts'
import { FOCUS_SCALE, TOKENS } from '../src/design/tokens.ts'
import type { LibraryItem } from '../src/core/contract.ts'

function item(overrides: Partial<LibraryItem> = {}): LibraryItem {
  return {
    id: 'item-1',
    workId: 'work-1',
    title: 'A Film',
    kind: 'movie',
    season: null,
    episode: null,
    year: 1995,
    relativePath: 'A Film (1995).mkv',
    sourceId: 'source-1',
    available: true,
    favorite: false,
    watched: false,
    hidden: false,
    positionMs: 0,
    durationMs: 0,
    updatedMs: 1,
    ...overrides,
  }
}

test('the grid fits the authored surface exactly, with no overflow', () => {
  // The same class of bug as the desktop's fixed four columns: a grid that is not
  // derived from the surface pushes its last card off the edge, and on a TV the edge
  // is the bezel.
  for (const columns of [1, 2, 3, 4, 5, 6]) {
    const geometry = gridFor(columns, 3)
    const lastX = cellPosition(columns - 1, geometry).x
    assert.ok(
      lastX + geometry.cardWidth <= APP_WIDTH,
      `${columns} columns overflow: ${lastX + geometry.cardWidth} > ${APP_WIDTH}`,
    )
  }
})

test('more columns means narrower cards, never overlapping ones', () => {
  const four = gridFor(4, 2)
  const five = gridFor(5, 2)
  assert.ok(five.cardWidth < four.cardWidth)
  const gapBetween = four.gapX - (four.cardWidth * (FOCUS_SCALE - 1))
  assert.ok(gapBetween >= 0, 'the gutter has to hold the focused card growing')
})

test('the gutter holds a focused card without touching its neighbour', () => {
  for (const columns of [3, 4, 5]) {
    const geometry = gridFor(columns, 2)
    const grown = geometry.cardWidth * FOCUS_SCALE
    const neighbour = geometry.cardWidth + geometry.gapX
    assert.ok(
      grown <= neighbour,
      `${columns} columns: a focused card (${grown}) overlaps its neighbour at ${neighbour}`,
    )
  }
})

test('a degenerate grid is still a grid', () => {
  // Zero or negative counts come from a layout that has not been measured yet.
  for (const columns of [0, -3, Number.NaN]) {
    const geometry = gridFor(columns, 0)
    assert.ok(Number.isInteger(geometry.cardWidth) && geometry.cardWidth > 0)
    assert.ok(Number.isInteger(geometry.cardHeight) && geometry.cardHeight > 0)
    assert.equal(geometry.columns, 1)
    assert.equal(geometry.rows, 1)
  }
})

test('cells are laid out row by row, and the second row starts one card lower', () => {
  const geometry = gridFor(5, 3)
  const first = cellPosition(0, geometry)
  const second = cellPosition(1, geometry)
  const wrapped = cellPosition(5, geometry)
  const other = cellPosition(-4, geometry)
  assert.equal(second.y, first.y)
  assert.equal(second.x - first.x, geometry.cardWidth + geometry.gapX)
  assert.equal(wrapped.x, first.x, 'the sixth card starts a new row')
  assert.equal(wrapped.y - first.y, geometry.cardHeight + geometry.gapY)
  assert.deepEqual(other, first, 'a negative index is brought back to the first cell')
})

test('the art is 16:9 and the card is taller than its art', () => {
  const geometry = gridFor(4, 2)
  assert.equal(geometry.artHeight, Math.round((geometry.artWidth * 9) / 16))
  assert.ok(geometry.cardHeight > geometry.artHeight, 'there is a title strip under the art')
})

test('a card is tall enough for both lines it draws, and a page fits the band it is given', () => {
  // The first TV build sized the title strip from a guess (`body * 2.4`) while the two
  // text lines under the art needed more, so the title and its subtitle were drawn past
  // the card's own bottom edge and were cut off. The same guess let a third row land
  // below the set. Both are geometry, so both are checked here rather than by eye.
  const bands = [
    { columns: 4, rows: 2, top: 232, bottom: 992 },
    { columns: 5, rows: 2, top: 204, bottom: 992 },
    { columns: 3, rows: 3, top: 32, bottom: 1048 },
  ]
  for (const band of bands) {
    const geometry = gridFor(band.columns, band.rows, TOKENS, APP_WIDTH, {
      top: band.top,
      bottom: band.bottom,
    })
    const lastLineBottom = geometry.subtitleY + Math.round(TOKENS.type.caption * 1.6)
    assert.ok(
      lastLineBottom <= geometry.cardHeight,
      `${band.columns}x${band.rows}: the sub-title ends at ${lastLineBottom}, past the card's ${geometry.cardHeight}`,
    )
    const last = cellPosition(band.columns * band.rows - 1, geometry)
    assert.ok(
      last.y + geometry.cardHeight <= band.bottom,
      `${band.columns}x${band.rows}: the last row ends at ${last.y + geometry.cardHeight}, past ${band.bottom}`,
    )
    assert.ok(last.y >= band.top, 'the first row starts inside the band')
    assert.ok(geometry.cardWidth > 0 && geometry.artHeight > 0)
  }
})

test('a card carries the exact values the template asks for', () => {
  const geometry = gridFor(3, 2)
  const cards = buildCards([item(), item({ id: 'item-2', title: 'Another' })], 0, geometry)
  assert.equal(cards.length, 2)
  const [first, second] = cards
  assert.equal(first?.focused, true)
  assert.equal(second?.focused, false)
  assert.equal(first?.scale, FOCUS_SCALE)
  assert.equal(second?.scale, 1)
  // The focused card is drawn on the focus surface, which is the design system's own
  // focus treatment rather than a ring or an outline.
  assert.match(first?.fill ?? '', /^#[0-9a-f]{6}$/)
  assert.notEqual(first?.fill, second?.fill)
  assert.equal(first?.width, geometry.cardWidth)
  assert.ok(Number.isInteger(first?.y))
  // No card may be drawn off the bottom of the authored surface.
  const last = cellPosition(5, geometry)
  assert.ok(last.y + geometry.cardHeight <= APP_HEIGHT, 'the last row is off screen')
})

test('progress is drawn only while it means something', () => {
  const geometry = gridFor(3, 1)
  const untouched = buildCards([item()], 0, geometry)[0]
  assert.equal(untouched?.progressVisible, false)
  assert.equal(untouched?.progressWidth, 0)

  const half = buildCards([item({ positionMs: 300_000, durationMs: 600_000 })], 0, geometry)[0]
  assert.equal(half?.progress, 0.5)
  assert.equal(half?.progressWidth, Math.round(geometry.artWidth * 0.5))
  assert.equal(half?.progressVisible, true)

  const done = buildCards([item({ positionMs: 600_000, durationMs: 600_000 })], 0, geometry)[0]
  assert.equal(done?.progressVisible, false, 'a finished item shows no bar')
})

test('the subtitle says the one thing the card cannot show', () => {
  assert.equal(subtitleFor(item()), '1995')
  assert.equal(subtitleFor(item({ kind: 'series', season: 2, episode: 4 })), 'S02E04')
  assert.equal(subtitleFor(item({ watched: true })), '1995 · Watched')
  assert.equal(
    subtitleFor(item({ positionMs: 60_000, durationMs: 600_000 })),
    '1995 · 9:00 left',
  )
  assert.equal(subtitleFor(item({ positionMs: 0, durationMs: 0, year: null })), 'Not started')
  // An unavailable item says why it cannot play, in place of the year: a card that
  // silently looked normal and then refused to play is the worse experience.
  assert.match(subtitleFor(item({ available: false })), /Not readable/)
})

test('a card for a file with no measured duration still reads honestly', () => {
  // The duration is unknown until the file has been played once, so the label must not
  // claim a countdown it cannot compute.
  const label = subtitleFor(item({ positionMs: 120_000, durationMs: 0 }))
  assert.equal(label, '1995 · 2:00')
})

test('the token scale the grid derives from is the one the desktop uses', () => {
  // If the TV scale were re-derived by hand, the grid would drift with it and nothing
  // would catch it: the card widths are integers computed from these numbers.
  assert.equal(TOKENS.type.body, 16 * 1.75)
  const geometry = gridFor(4, 2)
  assert.equal(geometry.originX, Math.round(TOKENS.spacing.shellPad))
})
