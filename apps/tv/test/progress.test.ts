import { test } from 'node:test'
import assert from 'node:assert/strict'

import type { PlaybackSnapshot } from '../src/core/contract.ts'
import {
  acceptProgress,
  isFinished,
  isWorthSaving,
  WORTH_SAVING_MS,
  type LiveSession,
} from '../src/core/progress.ts'

const live: LiveSession = { sessionId: 'session-a', generation: 1 }

function snapshot(overrides: Partial<PlaybackSnapshot> = {}): PlaybackSnapshot {
  return {
    sessionId: 'session-a',
    generation: 1,
    sequence: 1,
    positionMs: 1000,
    durationMs: 600_000,
    ended: false,
    paused: false,
    ...overrides,
  }
}

test('the first snapshot of a live session is accepted', () => {
  const verdict = acceptProgress(live, null, snapshot())
  assert.equal(verdict.accept, true)
})

test('a late callback from an ended session never overwrites live progress', () => {
  // The bug this rule exists for: an old session's sequence counter can be
  // arbitrarily high, so "newer number wins" would let the previous film's
  // playhead overwrite the one on screen.
  const current = snapshot({ sequence: 2, positionMs: 2000 })
  const stale = snapshot({ sessionId: 'session-ended', sequence: 99_999, positionMs: 500 })
  const verdict = acceptProgress(live, current, stale)
  assert.equal(verdict.accept, false)
  assert.equal(verdict.accept === false ? verdict.reason : '', 'stale_session')
  assert.equal(verdict.snapshot.positionMs, 2000, 'the stored position must stand')
})

test('a re-ordered callback cannot move the playhead backwards', () => {
  const current = snapshot({ sequence: 7, positionMs: 7000 })
  const late = snapshot({ sequence: 6, positionMs: 6000 })
  const verdict = acceptProgress(live, current, late)
  assert.equal(verdict.accept, false)
  assert.equal(verdict.accept === false ? verdict.reason : '', 'stale_sequence')
})

test('a repeated sequence number is refused rather than written twice', () => {
  const current = snapshot({ sequence: 4, positionMs: 4000 })
  const verdict = acceptProgress(live, current, snapshot({ sequence: 4, positionMs: 4000 }))
  assert.equal(verdict.accept, false)
  assert.equal(verdict.accept === false ? verdict.reason : '', 'same_sequence')
})

test('reloading the file in the same session restarts the sequence scale', () => {
  // Episode 2 of the same session: generation moves, so the sequence counter is
  // allowed to be lower than episode 1's without being stale.
  const first = snapshot({ generation: 1, sequence: 500, positionMs: 500_000 })
  const second = snapshot({ generation: 2, sequence: 1, positionMs: 1200 })
  const verdict = acceptProgress(live, first, second)
  assert.equal(verdict.accept, true)
  assert.equal(verdict.snapshot.positionMs, 1200)
})

test('a generation from the past is refused', () => {
  const current = snapshot({ generation: 3, sequence: 2 })
  const old = snapshot({ generation: 2, sequence: 900 })
  const verdict = acceptProgress(live, current, old)
  assert.equal(verdict.accept, false)
  assert.equal(verdict.accept === false ? verdict.reason : '', 'stale_generation')
})

test('a malformed snapshot is refused, and cannot be stored as progress', () => {
  const bad = snapshot({ positionMs: Number.NaN })
  const verdict = acceptProgress(live, null, bad)
  assert.equal(verdict.accept, false)
  assert.equal(verdict.accept === false ? verdict.reason : '', 'malformed')

  const negative = snapshot({ positionMs: -5 })
  assert.equal(acceptProgress(live, null, negative).accept, false)

  const fractional = snapshot({ sequence: 1.5 })
  assert.equal(acceptProgress(live, null, fractional).accept, false)
})

test('a stored value that cannot be trusted does not block a good one', () => {
  const broken = snapshot({ positionMs: Number.NaN })
  const verdict = acceptProgress(live, broken, snapshot({ sequence: 2 }))
  assert.equal(verdict.accept, true)
})

test('a finished file is finished whether the player said so or the ratio did', () => {
  assert.equal(isFinished(600_000, 600_000, true), true)
  assert.equal(isFinished(599_000, 600_000, false), true, 'past the ratio')
  assert.equal(isFinished(300_000, 600_000, false), false)
  // An unknown duration cannot make a file look finished.
  assert.equal(isFinished(5000, 0, false), false)
})

test('a position is only worth writing once it means something', () => {
  assert.equal(isWorthSaving(0, 600_000), false)
  assert.equal(isWorthSaving(5_000, 600_000), false, 'the studio logo is not a resume point')
  assert.equal(isWorthSaving(WORTH_SAVING_MS, 600_000), true)
  // A short clip: one percent is the floor, not fifteen seconds, or a 2-minute clip
  // would keep nothing until it was almost over.
  assert.equal(isWorthSaving(1_000, 60_000), true)
  assert.equal(isWorthSaving(10, 60_000), false)
})
