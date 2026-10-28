import { test } from 'node:test'
import assert from 'node:assert/strict'

import type { LibraryItem, Source } from '../src/core/contract.ts'
import {
  baseName,
  classifyEntry,
  decodeScanReport,
  describeScan,
  joinEntry,
  MAX_ENTRIES,
  MAX_ISSUES,
  mayDescend,
  planCatalog,
  reconcileSources,
  replaceSourceItems,
  withSourceAvailability,
  withinEntryLimit,
  Issues,
  NEVER_SCANNED,
} from '../src/core/scan.ts'

/** A deterministic id generator, so a plan can be compared between calls. */
function counter(): (prefix: string) => string {
  let n = 0
  return (prefix: string) => {
    n += 1
    return `${prefix}-${n}`
  }
}

/** One stored item, with the fields a test does not care about filled in. */
function item(overrides: Partial<LibraryItem> & { relativePath: string }): LibraryItem {
  return {
    id: 'item-1',
    workId: 'work-1',
    title: 'Untitled',
    kind: 'other',
    season: null,
    episode: null,
    year: null,
    sourceId: 'source-a',
    available: true,
    favorite: false,
    watched: false,
    hidden: false,
    positionMs: 0,
    durationMs: 0,
    updatedMs: 0,
    ...overrides,
  }
}

test('an entry is classified by what it is, not by what it looks like', () => {
  assert.equal(classifyEntry('Movies', true), 'directory')
  assert.equal(classifyEntry('.Trashes', true), 'hidden_directory')
  assert.equal(classifyEntry('Film.mkv', false), 'media')
  assert.equal(classifyEntry('Film.MKV', false), 'media')
  // A hidden file with a media extension is catalogued, exactly as the desktop does:
  // the two clients are supposed to show the same library.
  assert.equal(classifyEntry('.hidden.mkv', false), 'media')
  assert.equal(classifyEntry('.DS_Store', false), 'ignored')
  assert.equal(classifyEntry('cover.jpg', false), 'ignored')
})

test('a half-copied file is reported, not silently skipped', () => {
  // `Film.mkv.part` has the extension `part`, so an extension test alone would ignore
  // it — and the owner would never learn that a download is stuck.
  assert.equal(classifyEntry('Film.mkv.part', false), 'downloading')
  assert.equal(classifyEntry('Film.mkv.crdownload', false), 'downloading')
  assert.equal(classifyEntry('Film.MKV.ARIA2', false), 'downloading')
})

test('the walk is bounded in both directions', () => {
  assert.equal(mayDescend(0), true)
  assert.equal(mayDescend(15), true)
  assert.equal(mayDescend(16), false)
  assert.equal(mayDescend(17), false)
  assert.equal(withinEntryLimit(MAX_ENTRIES), true)
  assert.equal(withinEntryLimit(MAX_ENTRIES + 1), false)
})

test('a listed entry is joined on whether it is a name or a path', () => {
  // The Tizen WebIDL says `names`, the guide beside it says paths. Both are accepted,
  // and neither can produce a doubled directory component.
  assert.equal(joinEntry('removable1/Movies', 'Film.mkv'), 'removable1/Movies/Film.mkv')
  assert.equal(joinEntry('removable1/Movies', 'removable1/Movies/Film.mkv'), 'removable1/Movies/Film.mkv')
  assert.equal(joinEntry('removable1', 'Movies'), 'removable1/Movies')
  assert.equal(joinEntry('removable1/', 'Movies'), 'removable1/Movies')
  assert.equal(baseName('removable1/Movies/Film.mkv'), 'Film.mkv')
  assert.equal(baseName('Film.mkv'), 'Film.mkv')
})

test('a rescan keeps every id it gave before, so progress stays attached', () => {
  const first = planCatalog({
    sourceId: 'source-a',
    found: [{ relativePath: 'Show/S01E01.mkv' }, { relativePath: 'Film.2021.mkv' }],
    existing: [],
    complete: true,
    nowMs: 1,
    newId: counter(),
  })
  const watched = first.items.map((entry) => ({ ...entry, positionMs: 600_000, durationMs: 1_200_000 }))
  const second = planCatalog({
    sourceId: 'source-a',
    found: [{ relativePath: 'Film.2021.mkv' }, { relativePath: 'Show/S01E01.mkv' }],
    existing: watched,
    complete: true,
    nowMs: 2,
    newId: counter(),
  })
  assert.equal(second.items.length, 2)
  for (const before of watched) {
    const after = second.items.find((entry) => entry.relativePath === before.relativePath)
    assert.ok(after !== undefined)
    assert.equal(after.id, before.id)
    assert.equal(after.workId, before.workId)
    assert.equal(after.positionMs, 600_000)
    assert.equal(after.available, true)
  }
})

test('a new episode joins the work its siblings are already in', () => {
  // One generator across both scans, the way the app runs it: a restarted generator
  // would be free to hand out an id that is already in the library.
  const ids = counter()
  const first = planCatalog({
    sourceId: 'source-a',
    found: [{ relativePath: 'Show/S01E01.mkv' }],
    existing: [],
    complete: true,
    nowMs: 1,
    newId: ids,
  })
  const second = planCatalog({
    sourceId: 'source-a',
    found: [{ relativePath: 'Show/S01E01.mkv' }, { relativePath: 'Show/S01E02.mkv' }],
    existing: first.items,
    complete: true,
    nowMs: 2,
    newId: ids,
  })
  const [one, two] = second.items
  assert.ok(one !== undefined && two !== undefined)
  assert.equal(one.workId, two.workId)
  assert.notEqual(one.id, two.id)
})

test('a file whose name cannot be read never joins another one', () => {
  const plan = planCatalog({
    sourceId: 'source-a',
    found: [{ relativePath: 'Camera/12345.mkv' }, { relativePath: 'Other/67890.mkv' }],
    existing: [],
    complete: true,
    nowMs: 1,
    newId: counter(),
  })
  const [one, two] = plan.items
  assert.ok(one !== undefined && two !== undefined)
  assert.equal(one.kind, 'other')
  assert.notEqual(one.workId, two.workId)
})

test('a complete walk withdraws availability and keeps the position', () => {
  const before = [
    item({ id: 'item-9', relativePath: 'Gone.mkv', positionMs: 300_000, watched: true }),
  ]
  const plan = planCatalog({
    sourceId: 'source-a',
    found: [],
    existing: before,
    complete: true,
    nowMs: 1,
    newId: counter(),
  })
  assert.equal(plan.items.length, 1)
  assert.equal(plan.withdrawn, 1)
  assert.equal(plan.items[0]?.available, false)
  assert.equal(plan.items[0]?.positionMs, 300_000)
  assert.equal(plan.items[0]?.watched, true)
})

test('an incomplete walk withdraws nothing at all', () => {
  // This is the rule that keeps a bounded or interrupted scan from making the library
  // look empty. Rows keep the availability they had.
  const before = [item({ id: 'item-9', relativePath: 'Gone.mkv', positionMs: 300_000 })]
  const plan = planCatalog({
    sourceId: 'source-a',
    found: [],
    existing: before,
    complete: false,
    nowMs: 1,
    newId: counter(),
  })
  assert.equal(plan.items.length, 1)
  assert.equal(plan.withdrawn, 0)
  assert.equal(plan.items[0]?.available, true)
})

test('an entry that would escape its root is refused with a code', () => {
  const plan = planCatalog({
    sourceId: 'source-a',
    found: [{ relativePath: '../Outside.mkv' }, { relativePath: '/etc/passwd.mkv' }],
    existing: [],
    complete: true,
    nowMs: 1,
    newId: counter(),
  })
  assert.equal(plan.items.length, 0)
  assert.deepEqual(
    plan.issues.map((issue) => issue.code),
    ['unsafe_path', 'unsafe_path'],
  )
})

test('a file that is not a container, or is still downloading, is reported', () => {
  const plan = planCatalog({
    sourceId: 'source-a',
    found: [
      { relativePath: 'Notes.txt' },
      { relativePath: 'Film.mkv.part' },
      { relativePath: 'Film.mkv' },
    ],
    existing: [],
    complete: true,
    nowMs: 1,
    newId: counter(),
  })
  assert.equal(plan.items.length, 1)
  assert.deepEqual(
    plan.issues.map((issue) => issue.code).sort(),
    ['still_downloading', 'unsupported_container'],
  )
})

test('the same path twice cannot produce two rows with one id', () => {
  const plan = planCatalog({
    sourceId: 'source-a',
    found: [{ relativePath: 'Film.mkv' }, { relativePath: 'Film.mkv' }],
    existing: [],
    complete: true,
    nowMs: 1,
    newId: counter(),
  })
  assert.equal(plan.items.length, 1)
  assert.equal(plan.imported, 1)
})

test('planning one source cannot touch another source', () => {
  const other = item({ id: 'item-b', relativePath: 'Other.mkv', sourceId: 'source-b' })
  const plan = planCatalog({
    sourceId: 'source-a',
    found: [{ relativePath: 'Film.mkv' }],
    existing: [item({ id: 'item-a', relativePath: 'Old.mkv', sourceId: 'source-a' }), other],
    complete: true,
    nowMs: 1,
    newId: counter(),
  })
  assert.equal(plan.items.some((entry) => entry.sourceId === 'source-b'), false)
  // The replaced library holds the new scan, the other drive's row untouched, and the
  // scanned drive's own row that the walk did not see — unavailable, never dropped.
  const stored = item({ id: 'item-a', relativePath: 'Old.mkv', sourceId: 'source-a' })
  const all = replaceSourceItems([stored, other], 'source-a', plan.items)
  assert.equal(all.length, 3)
  assert.equal(new Set(all.map((entry) => entry.id)).size, 3)
  assert.equal(all.find((entry) => entry.id === 'item-b')?.available, true)
  assert.equal(all.find((entry) => entry.id === 'item-a')?.available, false)
})

test('a disconnected drive keeps its rows and becomes unavailable', () => {
  const items = [
    item({ id: 'item-a', relativePath: 'A.mkv', sourceId: 'source-a' }),
    item({ id: 'item-b', relativePath: 'B.mkv', sourceId: 'source-b' }),
  ]
  const updated = withSourceAvailability(items, 'source-a', false)
  assert.equal(updated.find((entry) => entry.id === 'item-a')?.available, false)
  assert.equal(updated.find((entry) => entry.id === 'item-b')?.available, true)
  // And back again, without anything having been lost in between.
  const restored = withSourceAvailability(updated, 'source-a', true)
  assert.equal(restored.find((entry) => entry.id === 'item-a')?.available, true)
})

test('sources are reconciled with what the TV reports, and never removed', () => {
  const sources: Source[] = [
    { id: 'source-removable1', label: 'removable1', rootPath: 'removable1', state: 'available', lastScanMs: 5 },
    { id: 'source-removable2', label: 'removable2', rootPath: 'removable2', state: 'available', lastScanMs: 6 },
  ]
  const reconciled = reconcileSources(
    sources,
    [
      { label: 'removable1', mounted: true },
      { label: 'removable3', mounted: true },
      { label: 'removable4', mounted: false },
    ],
    (label) => label,
  )
  assert.equal(reconciled.length, 3)
  assert.equal(reconciled.find((source) => source.label === 'removable1')?.state, 'available')
  // Gone from the platform's list: disconnected, with the row still there.
  assert.equal(reconciled.find((source) => source.label === 'removable2')?.state, 'missing')
  assert.equal(reconciled.find((source) => source.label === 'removable3')?.state, 'available')
  // A drive the platform lists as present but not mounted is not a source yet.
  assert.equal(reconciled.some((source) => source.label === 'removable4'), false)
  // Nothing is ever removed by reconciliation.
  assert.equal(reconciled.some((source) => source.label === 'removable2'), true)
})

test('the issue list truncates itself and says so', () => {
  const issues = new Issues()
  for (let index = 0; index < MAX_ISSUES + 10; index += 1) {
    issues.push(`File-${String(index)}.mkv`, 'unsupported_container')
  }
  const snapshot = issues.snapshot()
  assert.equal(snapshot.length, MAX_ISSUES + 1)
  assert.equal(snapshot[snapshot.length - 1]?.code, 'additional_errors_omitted')
})

test('a scan report says what happened in words', () => {
  assert.equal(describeScan(NEVER_SCANNED), 'No scan has run yet.')
  assert.match(
    describeScan({
      scannedSources: 1,
      imported: 12,
      withdrawn: 0,
      issues: [],
      complete: true,
      bounded: false,
      finishedMs: 1,
      reason: null,
    }),
    /12 files found on 1 drive, with nothing skipped/,
  )
  assert.match(
    describeScan({
      scannedSources: 1,
      imported: 12,
      withdrawn: 0,
      issues: [{ relativePath: 'A.part', code: 'still_downloading' }],
      complete: true,
      bounded: false,
      finishedMs: 1,
      reason: null,
    }),
    /1 entry skipped/,
  )
  assert.match(
    describeScan({
      scannedSources: 1,
      imported: 12,
      withdrawn: 3,
      issues: [],
      complete: false,
      bounded: true,
      finishedMs: 1,
      reason: null,
    }),
    /did not finish, so nothing was marked unreadable/,
  )
})

test('a stored scan report is read back, and junk is refused rather than believed', () => {
  const report = {
    scannedSources: 2,
    imported: 40,
    withdrawn: 3,
    issues: [{ relativePath: 'Film.mkv.part', code: 'still_downloading' }],
    complete: false,
    bounded: true,
    finishedMs: 1_700_000_000_000,
    reason: null,
  }
  assert.deepEqual(decodeScanReport(report), report)

  // A report is evidence. One that cannot be read says "nothing has been scanned"
  // rather than showing a file count nobody can account for.
  assert.deepEqual(decodeScanReport(null), NEVER_SCANNED)
  assert.deepEqual(decodeScanReport('lots'), NEVER_SCANNED)
  assert.deepEqual(decodeScanReport([]), NEVER_SCANNED)
  assert.deepEqual(decodeScanReport({}), {
    scannedSources: 0,
    imported: 0,
    withdrawn: 0,
    issues: [],
    complete: false,
    bounded: false,
    finishedMs: 0,
    reason: null,
  })
})

test('a stored report drops entries it cannot read and clamps impossible counts', () => {
  const decoded = decodeScanReport({
    scannedSources: -4,
    imported: 3.5,
    withdrawn: 2,
    complete: 'yes',
    bounded: 1,
    issues: [{ relativePath: 'A.mkv', code: 'unsupported_container' }, 'nonsense', { code: 7 }],
    reason: 'the drive would not list',
  })
  assert.equal(decoded.scannedSources, 0, 'a negative count is not a count')
  assert.equal(decoded.imported, 0, 'a fractional count is not a count')
  assert.equal(decoded.withdrawn, 2)
  assert.equal(decoded.complete, false, 'only a real true is complete')
  assert.equal(decoded.bounded, false)
  assert.deepEqual(decoded.issues, [{ relativePath: 'A.mkv', code: 'unsupported_container' }])
  assert.equal(decoded.reason, 'the drive would not list')
})
