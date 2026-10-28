import { test } from 'node:test'
import assert from 'node:assert/strict'

import { buildPlayerMenu, MENU_SEEK_MS } from '../src/core/player-menu.ts'
import type { TrackChoice } from '../src/core/tracks.ts'

const AUDIO: TrackChoice = { type: 'AUDIO', index: 0, language: 'eng', label: 'English' }
const TEXT: TrackChoice = { type: 'TEXT', index: 2, language: 'eng', label: 'English subtitles' }

test('every file gets controls and a way out, even with no tracks at all', () => {
  const rows = buildPlayerMenu({
    tracks: [],
    state: { audioIndex: null, textIndex: null, silent: true },
    paused: false,
    finished: false,
    nextLabel: '',
  })
  assert.deepEqual(
    rows.map((row) => row.kind),
    ['playpause', 'seekBack', 'seekForward', 'exit'],
  )
  assert.equal(new Set(rows.map((row) => row.key)).size, rows.length)
})

test('a file that never started gets no transport rows, only the way out', () => {
  // A Pause row over a black screen is a control that looks operable and does nothing.
  const rows = buildPlayerMenu({
    tracks: [],
    state: { audioIndex: null, textIndex: null, silent: true },
    paused: false,
    running: false,
    finished: false,
    nextLabel: '',
  })
  assert.deepEqual(
    rows.map((row) => row.kind),
    ['exit'],
  )
})

test('the first row says what OK will do', () => {
  const playing = buildPlayerMenu({
    tracks: [],
    state: { audioIndex: null, textIndex: null, silent: true },
    paused: false,
    finished: false,
    nextLabel: '',
  })
  assert.equal(playing[0]?.label, 'Pause')

  const paused = buildPlayerMenu({
    tracks: [],
    state: { audioIndex: null, textIndex: null, silent: true },
    paused: true,
    finished: false,
    nextLabel: '',
  })
  assert.equal(paused[0]?.label, 'Play')
})

test('the seek rows name the distance they move', () => {
  const rows = buildPlayerMenu({
    tracks: [],
    state: { audioIndex: null, textIndex: null, silent: true },
    paused: false,
    finished: false,
    nextLabel: '',
  })
  const seconds = MENU_SEEK_MS / 1000
  assert.equal(rows.find((row) => row.kind === 'seekBack')?.label, `Back ${seconds} seconds`)
  assert.equal(rows.find((row) => row.kind === 'seekForward')?.label, `Forward ${seconds} seconds`)
})

test('the file’s own tracks come after the controls, in the order a viewer thinks of them', () => {
  const rows = buildPlayerMenu({
    tracks: [AUDIO, TEXT],
    state: { audioIndex: 0, textIndex: null, silent: true },
    paused: false,
    finished: false,
    nextLabel: '',
  })
  const kinds = rows.map((row) => row.kind)
  assert.deepEqual(kinds.slice(0, 3), ['playpause', 'seekBack', 'seekForward'])
  assert.deepEqual(kinds.slice(3, 6), ['audio', 'off', 'text'])
  assert.equal(kinds[kinds.length - 1], 'exit', 'the way out is last and always present')
  assert.equal(rows.find((row) => row.kind === 'audio')?.selected, true)
  assert.equal(rows.find((row) => row.kind === 'off')?.selected, true)
})

test('a next episode is offered only once the file has ended, and only when one exists', () => {
  const base = {
    tracks: [],
    state: { audioIndex: null, textIndex: null, silent: true },
    paused: false,
  }
  const ended = buildPlayerMenu({ ...base, finished: true, nextLabel: 'S1 E2 · Second' })
  assert.ok(ended.some((row) => row.kind === 'next'), 'an ended episode offers the next one')

  const unfinished = buildPlayerMenu({ ...base, finished: false, nextLabel: 'S1 E2 · Second' })
  assert.equal(unfinished.some((row) => row.kind === 'next'), false)

  const nothingNext = buildPlayerMenu({ ...base, finished: true, nextLabel: '' })
  assert.equal(nothingNext.some((row) => row.kind === 'next'), false)
})
