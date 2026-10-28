import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

/**
 * A wiring check for the remote.
 *
 * The engine only delivers a key to a component whose `input` block names it. A screen
 * can implement `left()`, `down()` and `enter()` perfectly and still be unreachable from
 * the remote if none of them is declared, and the failure is silent: the keys simply
 * arrive and go nowhere. That is exactly what happened in the first build of this
 * client, which is why this test exists.
 *
 * It reads the screen sources rather than importing them, because importing a Blits
 * component needs the engine, and the thing under test is the declaration, not the
 * drawing. What it cannot do is prove that the TV delivers the key at all — that is a
 * device gate, and it is recorded as one.
 */
function inputBlock(file: string): string {
  const source = readFileSync(new URL(`../src/screens/${file}`, import.meta.url), 'utf8')
  const start = source.indexOf('\n  input: {')
  assert.notEqual(start, -1, `${file} declares no input block, so no remote key reaches it`)
  const end = source.indexOf('\n  },\n', start)
  assert.notEqual(end, -1, `${file} has an unterminated input block`)
  return source.slice(start, end)
}

const REQUIRED: Readonly<Record<string, readonly string[]>> = {
  'Home.ts': ['left', 'right', 'up', 'down', 'enter', 'info'],
  'Browse.ts': ['left', 'right', 'up', 'down', 'enter', 'info', 'rewind', 'fastForward'],
  'Detail.ts': ['up', 'down', 'enter'],
  'Settings.ts': ['up', 'down', 'enter'],
  // Back is the one key a screen may claim from the router, and the player must: a Back
  // press closes the track menu before it leaves the file.
  'Player.ts': [
    'enter',
    'space',
    'play',
    'pause',
    'playPause',
    'left',
    'right',
    'up',
    'down',
    'rewind',
    'fastForward',
    'info',
    'back',
  ],
}

for (const [file, keys] of Object.entries(REQUIRED)) {
  test(`${file} declares every remote key it implements`, () => {
    const block = inputBlock(file)
    for (const key of keys) {
      assert.ok(
        block.includes(`${key}() {`),
        `${file} does not declare the ${key} key, so pressing it would do nothing`,
      )
    }
  })
}

test('the player claims Back so the track menu can close before the file does', () => {
  const block = inputBlock('Player.ts')
  assert.ok(block.includes('back() {'), 'the player must handle Back itself')
  assert.ok(block.includes('this.$router.back()'), 'and still leave the file afterwards')
})
