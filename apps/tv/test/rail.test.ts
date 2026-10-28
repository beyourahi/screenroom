import { test } from 'node:test'
import assert from 'node:assert/strict'

import { RAIL_ENTRIES } from '../src/core/nav.ts'
import { railLayout, railRows, RAIL_WIDTH } from '../src/design/rail.ts'
import { hex, TOKENS } from '../src/design/tokens.ts'

test('the rail draws every destination, in order, inside its own column', () => {
  const rows = railRows(0, 0, true)
  assert.equal(rows.length, RAIL_ENTRIES.length)
  assert.deepEqual(
    rows.map((row) => row.label),
    RAIL_ENTRIES.map((entry) => entry.label),
  )
  for (const [index, row] of rows.entries()) {
    assert.ok(row.x >= 0 && row.x + row.width <= RAIL_WIDTH, `${row.label} is inside the rail`)
    assert.ok(row.y + row.height < 1080, `${row.label} is on the screen`)
    assert.ok(row.labelX > 0 && row.labelX < row.width, `${row.label}'s ink fits its row`)
    if (index > 0) {
      assert.ok(row.y > rows[index - 1]!.y, 'rows are laid out downwards')
    }
  }
})

test('the destination the viewer is on and the row the remote is on are different facts', () => {
  const focused = railRows(1, 1, true)
  assert.equal(focused[1]?.focused, true)
  assert.equal(focused[1]?.fill, hex(TOKENS.palette.focusSurface))
  assert.equal(focused[1]?.labelColor, hex(TOKENS.palette.focusForeground))

  const elsewhere = railRows(1, 4, true)
  assert.equal(elsewhere[1]?.active, true, 'Movies still says "you are here"')
  assert.equal(elsewhere[1]?.focused, false)
  assert.notEqual(elsewhere[1]?.fill, elsewhere[4]?.fill, 'the two states look different')
  assert.equal(elsewhere[4]?.focused, true)
})

test('the rail dims itself when the grid owns the cursor', () => {
  const rows = railRows(2, 0, false)
  assert.equal(rows.some((row) => row.focused), false)
  assert.equal(rows[2]?.active, true, 'the destination is still marked')
})

test('the rail never gets wider than the room the grid leaves for it', () => {
  const layout = railLayout()
  assert.ok(layout.rowX + layout.rowWidth <= RAIL_WIDTH)
  assert.ok(layout.rowHeight >= TOKENS.control.md, 'a rail row is a comfortable hit target')
  assert.ok(layout.labelSize >= 24, 'the menu is legible from a sofa')
})
