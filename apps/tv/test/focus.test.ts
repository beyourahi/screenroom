import { test } from 'node:test'
import assert from 'node:assert/strict'

import { clampIndex, createFocusMemory, moveFocus, restoreFocus } from '../src/core/focus.ts'

/** The desktop's own rule, restated here so a divergence fails a test rather than a user. */
function desktopMove(
  index: number,
  count: number,
  step: number,
  direction: 'left' | 'right' | 'up' | 'down',
): number {
  const current = Math.min(Math.max(index, 0), count - 1)
  const next =
    direction === 'left'
      ? current > 0
        ? current - 1
        : undefined
      : direction === 'right'
        ? current + 1 < count
          ? current + 1
          : undefined
        : direction === 'up'
          ? current - step >= 0
            ? current - step
            : undefined
          : current + step < count
            ? current + step
            : undefined
  return next ?? current
}

test('the TV rule is the desktop rule, index for index', () => {
  // Parity is the point: the same owner uses both devices, and an arrow key that
  // behaves differently on the sofa is a bug people blame themselves for.
  for (const count of [1, 5, 9, 18]) {
    for (const columns of [1, 3, 5]) {
      for (let index = 0; index < count; index += 1) {
        for (const direction of ['left', 'right', 'up', 'down'] as const) {
          assert.equal(
            moveFocus(index, count, columns, direction),
            desktopMove(index, count, columns, direction),
            `${direction} from ${index} of ${count} at ${columns} columns`,
          )
        }
      }
    }
  }
})

test('left and right move one item, crossing a row boundary at the ends', () => {
  // A 3-wide grid of 9 items. The cursor is a flat index, so index 3 is the first
  // item of the second row and left from it is the last item of the first row.
  assert.equal(moveFocus(0, 9, 3, 'right'), 1)
  assert.equal(moveFocus(2, 9, 3, 'right'), 3)
  assert.equal(moveFocus(3, 9, 3, 'left'), 2)
})

test('up and down move a whole row using the drawn column count', () => {
  assert.equal(moveFocus(0, 9, 3, 'down'), 3)
  assert.equal(moveFocus(4, 9, 3, 'up'), 1)
  // The same press on a 5-wide grid lands somewhere else, which is why the caller
  // must pass the count the grid was drawn with.
  assert.equal(moveFocus(0, 9, 5, 'down'), 5)
})

test('a press that would leave the list does nothing, and never clamps sideways', () => {
  // Clamping up from the first row to index 0 would move the cursor sideways when the
  // user pressed up, which is the kind of thing that feels like the remote is broken.
  assert.equal(moveFocus(1, 9, 3, 'up'), 1, 'there is nothing above the first row')
  assert.equal(moveFocus(8, 9, 3, 'down'), 8, 'there is nothing below the last row')
  assert.equal(moveFocus(7, 9, 3, 'down'), 7)
  assert.equal(moveFocus(0, 9, 3, 'left'), 0)
  assert.equal(moveFocus(8, 9, 3, 'right'), 8)
})

test('moving right then left returns to where it started', () => {
  let index = 2
  index = moveFocus(index, 12, 4, 'right')
  assert.equal(index, 3)
  index = moveFocus(index, 12, 4, 'left')
  assert.equal(index, 2)
})

test('focus never leaves the grid when the grid shrinks under it', () => {
  assert.equal(clampIndex(9, 4), 3)
  assert.equal(clampIndex(-3, 4), 0)
  assert.equal(clampIndex(0, 0), -1, 'nothing to focus')
  assert.equal(moveFocus(0, 0, 3, 'down'), -1)
  // An index that is already out of range is brought back onto the grid before the
  // press is read, so the press moves from the last card rather than from index 99.
  assert.equal(moveFocus(99, 4, 3, 'left'), 2)
  assert.equal(moveFocus(99, 4, 3, 'up'), 0)
})

test('a bad column count degrades to a list rather than a NaN index', () => {
  for (const columns of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
    const moved = moveFocus(1, 9, columns, 'down')
    assert.ok(Number.isInteger(moved), `columns=${columns} produced ${moved}`)
    assert.equal(moved, 2)
  }
})

test('focus is remembered per screen and restored clamped', () => {
  const memory = createFocusMemory()
  assert.equal(restoreFocus(memory, '/browse', 10), 0, 'a fresh screen starts at the first item')

  memory.set('/browse', 6, 10)
  memory.set('/home', 2, 5)
  assert.equal(restoreFocus(memory, '/browse', 10), 6)
  assert.equal(restoreFocus(memory, '/home', 5), 2)

  // A rescan that removes files must not leave focus pointing past the end.
  assert.equal(restoreFocus(memory, '/browse', 3), 2)
  // And a screen with nothing in it has nowhere to focus.
  assert.equal(restoreFocus(memory, '/browse', 0), -1)

  memory.clear('/home')
  assert.equal(restoreFocus(memory, '/home', 5), 0)
})
