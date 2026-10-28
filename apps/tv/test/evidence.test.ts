import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  appendEvidence,
  decodeEvidence,
  encodeEvidence,
  MAX_DETAIL,
  MAX_ENTRIES,
  sanitise,
  summariseEvidence,
  type EvidenceEntry,
} from '../src/core/evidence.ts'

function entry(overrides: Partial<EvidenceEntry> = {}): EvidenceEntry {
  return { atMs: 1000, tag: 'boot', detail: 'started', ...overrides }
}

test('sanitise flattens tabs and newlines so one entry stays one line', () => {
  // A tab or newline is the record separator. If one survived, the reader would split
  // the entry in two and the log would report an event that never happened.
  assert.equal(sanitise('a\tb\nc\r\nd'), 'a b c d')
})

test('sanitise removes other control characters', () => {
  const withBell = ['bell', String.fromCharCode(7), 'here'].join('')
  assert.equal(sanitise(withBell), 'bellhere')
})

test('sanitise truncates an over-long detail and says so', () => {
  const long = 'x'.repeat(MAX_DETAIL * 2)
  const cut = sanitise(long)
  assert.equal(cut.length, MAX_DETAIL)
  assert.ok(cut.endsWith('…'))
})

test('appendEvidence keeps order and caps the log from the front', () => {
  let log: EvidenceEntry[] = []
  for (let index = 0; index < 5; index += 1) {
    log = appendEvidence(log, entry({ atMs: index, detail: `event ${index}` }), 3)
  }
  assert.deepEqual(
    log.map((item) => item.detail),
    ['event 2', 'event 3', 'event 4'],
  )
})

test('appendEvidence defaults to MAX_ENTRIES', () => {
  let log: EvidenceEntry[] = []
  for (let index = 0; index < MAX_ENTRIES + 10; index += 1) {
    log = appendEvidence(log, entry({ detail: `${index}` }))
  }
  assert.equal(log.length, MAX_ENTRIES)
  assert.equal(log[log.length - 1]?.detail, `${MAX_ENTRIES + 9}`)
})

test('appendEvidence refuses an entry with no tag', () => {
  // An unnamed event cannot be reported on, and an empty first field would make the
  // line ambiguous when read back.
  const log = appendEvidence([], entry({ tag: '' }))
  assert.deepEqual(log, [])
})

test('appendEvidence replaces a non-finite timestamp rather than storing NaN', () => {
  const log = appendEvidence([], entry({ atMs: Number.NaN }))
  assert.equal(log[0]?.atMs, 0)
})

test('the log round-trips through text', () => {
  const log = [entry({ atMs: 1, tag: 'boot', detail: 'a b c' }), entry({ atMs: 2, tag: 'scan', detail: '2 files' })]
  assert.deepEqual(decodeEvidence(encodeEvidence(log)), log)
})

test('decodeEvidence reads nothing as nothing, not as a failure', () => {
  assert.deepEqual(decodeEvidence(null), [])
  assert.deepEqual(decodeEvidence(''), [])
})

test('decodeEvidence drops a damaged line and keeps the rest', () => {
  // A half-written record is the realistic case: the TV was switched off mid-write.
  const text = ['10\tboot\tok', 'not a record', '20\tscan\t3 files', 'x\ttag\tdetail'].join('\n')
  assert.deepEqual(decodeEvidence(text), [
    { atMs: 10, tag: 'boot', detail: 'ok' },
    { atMs: 20, tag: 'scan', detail: '3 files' },
  ])
})

test('decodeEvidence keeps a detail that itself contains a tab-shaped character', () => {
  // A detail is sanitised on write and again on read, so a record pulled from a device
  // that was written by an older build cannot smuggle a separator in.
  const text = '10\tboot\ta\tb'
  assert.deepEqual(decodeEvidence(text), [{ atMs: 10, tag: 'boot', detail: 'a b' }])
})

test('summariseEvidence says plainly when nothing was recorded', () => {
  assert.match(summariseEvidence([]), /Nothing has been recorded/)
  assert.equal(summariseEvidence([entry({ tag: 'scan', detail: '4 files' })]), '1 entries, last scan: 4 files')
})
