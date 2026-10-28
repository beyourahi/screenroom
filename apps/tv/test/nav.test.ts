import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  createNavMemory,
  gridWindow,
  moveNav,
  RAIL_ENTRIES,
  railIndexFor,
  restoreNav,
  type NavBounds,
} from '../src/core/nav.ts'

const BOUNDS: NavBounds = { railCount: RAIL_ENTRIES.length, contentCount: 12, columns: 5 }

test('the rail holds every destination the app has, once each', () => {
  assert.deepEqual(
    RAIL_ENTRIES.map((entry) => entry.label),
    ['Home', 'Movies', 'TV Shows', 'Search', 'Media Sources', 'Settings'],
  )
  assert.equal(new Set(RAIL_ENTRIES.map((entry) => entry.route)).size, RAIL_ENTRIES.length)
  assert.equal(new Set(RAIL_ENTRIES.map((entry) => entry.key)).size, RAIL_ENTRIES.length)
})

test('a route maps onto the destination it belongs to', () => {
  assert.equal(railIndexFor('/'), 0)
  assert.equal(railIndexFor('/movies'), 1)
  assert.equal(railIndexFor('/series'), 2)
  assert.equal(railIndexFor('/search'), 3)
  assert.equal(railIndexFor('/sources'), 4)
  assert.equal(railIndexFor('/settings'), 5)
  assert.equal(railIndexFor('/movies/'), 1, 'a trailing slash is still Movies')
  assert.equal(railIndexFor('/detail'), -1, 'a leaf screen belongs to no destination')
  assert.equal(railIndexFor('/player'), -1)
})

test('the rail moves up and down and stops at both ends', () => {
  const top = { zone: 'rail' as const, rail: 0, content: 0 }
  assert.equal(moveNav(top, 'up', BOUNDS).rail, 0, 'up at the top does nothing')
  assert.equal(moveNav(top, 'down', BOUNDS).rail, 1)
  const bottom = { zone: 'rail' as const, rail: RAIL_ENTRIES.length - 1, content: 0 }
  assert.equal(moveNav(bottom, 'down', BOUNDS).rail, RAIL_ENTRIES.length - 1)
  assert.equal(moveNav(bottom, 'up', BOUNDS).rail, RAIL_ENTRIES.length - 2)
})

test('right enters the grid at the card the viewer left, not the first one', () => {
  const parked = { zone: 'rail' as const, rail: 2, content: 7 }
  const inside = moveNav(parked, 'right', BOUNDS)
  assert.equal(inside.zone, 'content')
  assert.equal(inside.content, 7, 'coming back from the rail returns to the same card')
  assert.equal(inside.rail, 2)
})

test('left on the first column returns to the rail', () => {
  const firstColumn = { zone: 'content' as const, rail: 1, content: 5 }
  const rail = moveNav(firstColumn, 'left', BOUNDS)
  assert.equal(rail.zone, 'rail')
  assert.equal(rail.content, 5, 'the card index is kept for the way back')

  // Anywhere else left is an ordinary one-card move, exactly as on the desktop.
  const secondColumn = moveNav({ zone: 'content', rail: 1, content: 6 }, 'left', BOUNDS)
  assert.equal(secondColumn.zone, 'content')
  assert.equal(secondColumn.content, 5)
})

test('the grid moves by a row and never wraps or clamps sideways', () => {
  const middle = { zone: 'content' as const, rail: 0, content: 6 }
  assert.equal(moveNav(middle, 'down', BOUNDS).content, 11)
  assert.equal(moveNav(middle, 'up', BOUNDS).content, 1)
  const lastRow = { zone: 'content' as const, rail: 0, content: 11 }
  assert.equal(moveNav(lastRow, 'down', BOUNDS).content, 11, 'down on the last row stays')
  const right = { zone: 'content' as const, rail: 0, content: 11 }
  assert.equal(moveNav(right, 'right', BOUNDS).content, 11, 'right on the last card stays')
})

test('with nothing catalogued the rail is the only zone', () => {
  const empty: NavBounds = { railCount: RAIL_ENTRIES.length, contentCount: 0, columns: 5 }
  for (const direction of ['left', 'right', 'up', 'down'] as const) {
    const next = moveNav({ zone: 'content', rail: 0, content: 0 }, direction, empty)
    assert.equal(next.zone, 'rail', `${direction} must not leave a cursor in an empty grid`)
    assert.equal(next.content, -1)
  }
})

test('a window shows whole rows and scrolls only when the cursor leaves them', () => {
  const first = gridWindow(1200, 5, 2, 0)
  assert.deepEqual(first, { first: 0, count: 10, rows: 2, scrollRows: 0, totalRows: 240 })

  const stillFirstPage = gridWindow(1200, 5, 2, 9)
  assert.equal(stillFirstPage.first, 0, 'the cursor can move inside the band without scrolling')

  const secondRow = gridWindow(1200, 5, 2, 10)
  assert.equal(secondRow.first, 5, 'the band slides by exactly one row')
  assert.equal(secondRow.scrollRows, 1)
  assert.equal(secondRow.count, 10)
})

test('the last window stops at the end and still contains the cursor', () => {
  const last = gridWindow(1200, 5, 2, 1199)
  assert.equal(last.first, 1190)
  assert.equal(last.count, 10)
  assert.ok(last.first <= 1199 && 1199 < last.first + last.count)
  assert.equal(last.scrollRows, 238)
})

test('a short list draws what it has, and an empty one draws nothing', () => {
  assert.deepEqual(gridWindow(3, 5, 2, 0), {
    first: 0,
    count: 3,
    rows: 1,
    scrollRows: 0,
    totalRows: 1,
  })
  assert.deepEqual(gridWindow(0, 5, 2, 0), {
    first: 0,
    count: 0,
    rows: 0,
    scrollRows: 0,
    totalRows: 0,
  })
})

test('a nonsensical layout degrades to a list rather than NaN', () => {
  const window = gridWindow(10, Number.NaN, 2, Number.NaN)
  assert.equal(window.totalRows, 10, 'a bad column count degrades to one card per row')
  assert.equal(window.first, 0, 'a bad cursor index starts at the first card')
  assert.equal(window.count, 2)
})

test('a remembered cursor is restored, and clamped when the list shrank', () => {
  const memory = createNavMemory()
  memory.set('/movies', { zone: 'content', rail: 1, content: 40 }, BOUNDS)
  const restored = restoreNav(memory, '/movies', BOUNDS, 'content')
  assert.deepEqual(restored, { zone: 'content', rail: 1, content: 11 })

  const shrunk = restoreNav(memory, '/movies', { ...BOUNDS, contentCount: 3 }, 'content')
  assert.equal(shrunk.content, 2, 'a rescan that removed files lands on the last card')
})

test('a screen opened for the first time starts where its caller says', () => {
  const memory = createNavMemory()
  assert.equal(restoreNav(memory, '/movies', BOUNDS, 'content').zone, 'content')
  assert.equal(restoreNav(memory, '/', BOUNDS, 'rail').zone, 'rail')
  assert.equal(restoreNav(memory, '/', BOUNDS, 'rail').rail, 0)
})

test('an empty list forces the rail even when the content was remembered', () => {
  const memory = createNavMemory()
  memory.set('/movies', { zone: 'content', rail: 1, content: 4 }, BOUNDS)
  const restored = restoreNav(memory, '/movies', { ...BOUNDS, contentCount: 0 }, 'content')
  assert.equal(restored.zone, 'rail')
  assert.equal(restored.content, -1)
})
