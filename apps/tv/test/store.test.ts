import { test } from 'node:test'
import assert from 'node:assert/strict'

import type { LibraryItem, PlaybackSnapshot, Source } from '../src/core/contract.ts'
import { createMemoryStorage, LibraryService, nextId } from '../src/core/storage.ts'
import {
  decodeSnapshot,
  emptySnapshot,
  encodeSnapshot,
  MAX_ROWS,
  STORE_FORMAT_VERSION,
  withHiddenWork,
  withItemProgress,
} from '../src/core/store.ts'

function item(overrides: Partial<LibraryItem> = {}): LibraryItem {
  return {
    id: 'item-1',
    workId: 'work-1',
    title: 'Film',
    kind: 'movie',
    season: null,
    episode: null,
    year: 1995,
    relativePath: 'Film.1995.mkv',
    sourceId: 'source-1',
    available: true,
    favorite: false,
    watched: false,
    hidden: false,
    positionMs: 0,
    durationMs: 600_000,
    updatedMs: 1,
    ...overrides,
  }
}

const source: Source = {
  id: 'source-1',
  label: 'USB drive',
  rootPath: '/USB/Media',
  state: 'available',
  lastScanMs: 1,
}

test('an empty document and a missing document both read as a fresh install', () => {
  for (const raw of [null, '']) {
    const result = decodeSnapshot(raw)
    assert.equal(result.reset, false)
    assert.deepEqual(result.issues, [])
    assert.equal(result.snapshot.formatVersion, STORE_FORMAT_VERSION)
  }
})

test('a round trip preserves the library', () => {
  const snapshot = {
    ...emptySnapshot(),
    sources: [source],
    items: [item()],
    profiles: [{ id: 'p1', name: 'Owner' }],
    activeProfileId: 'p1',
    hiddenWorkIds: ['work-9'],
  }
  const read = decodeSnapshot(encodeSnapshot(snapshot))
  assert.equal(read.reset, false)
  assert.deepEqual(read.issues, [])
  assert.deepEqual(read.snapshot, snapshot)
})

test('one unreadable row is dropped and counted, not the whole library', () => {
  // The owner's history exists nowhere else, so a single bad row must never cost
  // everything. The row is refused and the count is reported.
  const document = JSON.stringify({
    formatVersion: STORE_FORMAT_VERSION,
    profiles: [{ id: 'p1', name: 'Owner' }],
    sources: [source],
    items: [item({ id: 'good' }), { id: 'no-title' }, item({ id: 'also-good' })],
    activeProfileId: 'p1',
    hiddenWorkIds: [],
  })
  const read = decodeSnapshot(document)
  assert.equal(read.reset, false)
  assert.deepEqual(
    read.snapshot.items.map((entry) => entry.id),
    ['good', 'also-good'],
  )
  assert.equal(read.issues.length, 1)
  assert.equal(read.issues[0]?.index, 1)
  assert.equal(read.issues[0]?.collection, 'items')
})

test('a document from a newer version is refused rather than guessed at', () => {
  const document = JSON.stringify({ formatVersion: STORE_FORMAT_VERSION + 1, items: [item()] })
  const read = decodeSnapshot(document)
  assert.equal(read.reset, true)
  assert.deepEqual(read.snapshot.items, [])
  assert.match(read.issues[0]?.reason ?? '', /newer version/)
})

test('damaged bytes do not stop the app from starting', () => {
  for (const raw of ['{ not json', '[]', '"a string"', '{"formatVersion": 1, "items": "no"}']) {
    const read = decodeSnapshot(raw)
    assert.ok(Array.isArray(read.snapshot.items), `${raw} must still produce a usable store`)
  }
  assert.equal(decodeSnapshot('{ not json').reset, true)
})

test('an oversized or over-long document is refused before it is parsed', () => {
  const huge = `{"formatVersion":1,"items":[${'"x",'.repeat(0)}]}`
  assert.equal(decodeSnapshot(huge).reset, false, 'a small document still reads')

  const tooLong = 'x'.repeat(9 * 1024 * 1024)
  assert.equal(decodeSnapshot(tooLong).reset, true)

  // The row ceiling is checked before any row is read, so the rows here are as small
  // as a row can be: the refusal is about how many there are, not about their size.
  const tooManyRows = JSON.stringify({
    formatVersion: STORE_FORMAT_VERSION,
    items: Array.from({ length: MAX_ROWS + 1 }, () => ({})),
  })
  const read = decodeSnapshot(tooManyRows)
  assert.deepEqual(read.snapshot.items, [])
  assert.match(read.issues[0]?.reason ?? '', /more than/)
})

test('a remembered profile that no longer exists is not handed back as current', () => {
  const document = JSON.stringify({
    formatVersion: STORE_FORMAT_VERSION,
    profiles: [{ id: 'p1', name: 'Owner' }],
    activeProfileId: 'p-gone',
  })
  assert.equal(decodeSnapshot(document).snapshot.activeProfileId, null)
})

test('a state change returns a new snapshot and leaves the old one alone', () => {
  const before = { ...emptySnapshot(), items: [item()] }
  const after = withItemProgress(before, 'item-1', 30_000, 600_000, false, 99)
  assert.equal(before.items[0]?.positionMs, 0, 'the previous snapshot must not be mutated')
  assert.equal(after.items[0]?.positionMs, 30_000)
  assert.equal(after.items[0]?.updatedMs, 99)
  // An id that is not there changes nothing, rather than adding a row.
  assert.equal(withItemProgress(before, 'missing', 1, 1, false, 1).items.length, 1)
})

test('hiding a resume point is idempotent', () => {
  const once = withHiddenWork(emptySnapshot(), 'work-1', true)
  const twice = withHiddenWork(once, 'work-1', true)
  assert.deepEqual(twice.hiddenWorkIds, ['work-1'])
  assert.deepEqual(withHiddenWork(once, 'work-1', false).hiddenWorkIds, [])
})

test('generated ids are stable-looking and never contain a path', () => {
  const id = nextId('profile', 3, 1_700_000_000_000)
  assert.match(id, /^profile-[0-9a-z]+-3$/)
  assert.ok(!id.includes('/') && !id.includes('\\'))
})

/** The session id the client hands out. Every callback has to carry this one back. */
const LIVE_SESSION = 'session-1'

function snapshotFor(sequence: number, positionMs: number): PlaybackSnapshot {
  return {
    sessionId: LIVE_SESSION,
    generation: 1,
    sequence,
    positionMs,
    durationMs: 600_000,
    ended: false,
    paused: false,
  }
}

test('progress is written, and a stale callback still cannot overwrite it', () => {
  const port = createMemoryStorage()
  const service = new LibraryService(port, () => 1000)
  service.replaceCatalog([item()], [source])
  service.beginSession(LIVE_SESSION, 'item-1')

  assert.equal(
    service.recordProgress({ sessionId: LIVE_SESSION, generation: 1 }, snapshotFor(2, 30_000), 'work-1'),
    true,
  )
  assert.equal(service.items()[0]?.positionMs, 30_000)

  // Sequence 1 arrives late, after 2 was accepted.
  assert.equal(
    service.recordProgress({ sessionId: LIVE_SESSION, generation: 1 }, snapshotFor(1, 5_000), 'work-1'),
    false,
  )
  assert.equal(service.items()[0]?.positionMs, 30_000, 'the playhead went backwards')

  // And a session that has ended writes nothing at all, even when the numbers look
  // newer: the id is what decides, not the sequence.
  service.endSession(LIVE_SESSION)
  assert.equal(
    service.recordProgress({ sessionId: LIVE_SESSION, generation: 1 }, snapshotFor(9, 900_000), 'work-1'),
    false,
  )
  assert.equal(service.items()[0]?.positionMs, 30_000)
})

test('a rescan never resets a position the scanner did not measure', () => {
  const service = new LibraryService(createMemoryStorage())
  service.replaceCatalog([item()], [source])
  service.beginSession(LIVE_SESSION, 'item-1')
  service.recordProgress({ sessionId: LIVE_SESSION, generation: 1 }, snapshotFor(1, 120_000), 'work-1')

  // The scanner reports the same file with no progress of its own, which is what it
  // always does: progress belongs to the owner, not to the walk.
  service.replaceCatalog([item({ positionMs: 0, durationMs: 0 })], [source])
  assert.equal(service.items()[0]?.positionMs, 120_000)
  assert.equal(service.items()[0]?.durationMs, 600_000, 'a measured duration is kept')
})

test('finishing a file clears the resume point and reports it as watched', () => {
  const service = new LibraryService(createMemoryStorage())
  service.replaceCatalog([item()], [source])
  service.beginSession(LIVE_SESSION, 'item-1')
  service.recordProgress({ sessionId: LIVE_SESSION, generation: 1 }, snapshotFor(1, 300_000), 'work-1')
  service.recordProgress(
    { sessionId: LIVE_SESSION, generation: 1 },
    { ...snapshotFor(2, 600_000), ended: true },
    'work-1',
  )
  assert.equal(service.items()[0]?.watched, true)
  assert.equal(service.items()[0]?.positionMs, 0, 'a finished file has no resume point')
})

test('a device that refuses a write says so instead of assuming it worked', () => {
  // Tizen's localStorage throws when the quota is full, so a full disk must not look
  // like a saved position.
  const port = createMemoryStorage()
  const service = new LibraryService(port)
  const full = {
    ...port,
    write: (): boolean => false,
  }
  const failing = new LibraryService(full)
  failing.replaceCatalog([item()], [source])
  assert.notEqual(failing.writeFailure(), null)
  assert.match(failing.writeFailure()?.reason ?? '', /refused/)
  assert.equal(service.writeFailure(), null)
})

test('an unchanged state is not written twice', () => {
  let writes = 0
  const port = createMemoryStorage()
  const counting = {
    ...port,
    write: (document: string) => {
      writes += 1
      return port.write(document)
    },
  }
  const service = new LibraryService(counting)
  service.replaceCatalog([item()], [source])
  const afterFirst = writes
  // The same catalog again: nothing changed, so nothing is written. A write per
  // frame would be felt in the frame budget on a 2021 TV.
  service.replaceCatalog([item()], [source])
  assert.equal(writes, afterFirst)
})

test('a reload reads back what was stored', () => {
  const port = createMemoryStorage()
  const first = new LibraryService(port, () => 5000)
  first.replaceCatalog([item()], [source])
  first.beginSession(LIVE_SESSION, 'item-1')
  first.recordProgress({ sessionId: LIVE_SESSION, generation: 1 }, snapshotFor(1, 90_000), 'work-1')

  const second = new LibraryService(port)
  assert.equal(second.items()[0]?.positionMs, 90_000)
  assert.equal(second.loadReport().lostStoredState, false)
  assert.deepEqual(second.loadReport().issues, [])
})

test('damaged stored state is reported on the load, not hidden', () => {
  const service = new LibraryService(createMemoryStorage('not json at all'))
  const report = service.loadReport()
  assert.equal(report.lostStoredState, true)
  assert.ok(report.issues.length > 0)
  assert.ok(report.location.length > 0, 'the report says where state lives')
})
