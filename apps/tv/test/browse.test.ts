import { test } from 'node:test'
import assert from 'node:assert/strict'

import type { LibraryItem } from '../src/core/contract.ts'
import {
  clampPage,
  continueWatching,
  emptyState,
  filterItems,
  formatPosition,
  kindOf,
  NO_FILTER,
  pageCount,
  pageItems,
  PAGE_SIZE,
  progressFraction,
} from '../src/core/browse.ts'

function item(overrides: Partial<LibraryItem> = {}): LibraryItem {
  return {
    id: 'item-1',
    workId: 'work-1',
    title: 'Film',
    kind: 'movie',
    season: null,
    episode: null,
    year: 1995,
    relativePath: 'Movies/Film.1995.mkv',
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

test('a page is a whole number of rows', () => {
  assert.equal(PAGE_SIZE % 5, 0)
})

test('paging covers every item exactly once and never ends mid-row', () => {
  const items = Array.from({ length: PAGE_SIZE + 3 }, (_value, index) =>
    item({ id: `item-${index}`, workId: `work-${index}` }),
  )
  const first = pageItems(items, 0)
  const second = pageItems(items, 1)
  assert.equal(first.length, PAGE_SIZE)
  assert.equal(second.length, 3)
  assert.equal(new Set([...first, ...second].map((entry) => entry.id)).size, items.length)
})

test('a page number outside the range is clamped rather than showing nothing', () => {
  const items = Array.from({ length: 4 }, (_value, index) => item({ id: `i${index}` }))
  assert.equal(clampPage(-2, items.length), 0)
  assert.equal(clampPage(99, items.length), 0, 'one page of items is page zero')
  assert.equal(pageCount(0), 1)
  assert.equal(pageCount(PAGE_SIZE), 1)
  assert.equal(pageCount(PAGE_SIZE + 1), 2)
})

test('the kind filter maps onto the stored kinds', () => {
  assert.equal(kindOf('all'), null)
  assert.equal(kindOf('movies'), 'movie')
  assert.equal(kindOf('series'), 'series')
})

test('search matches the title and the filename', () => {
  const items = [
    item({ id: 'a', title: 'Heat' }),
    item({ id: 'b', title: 'Show', kind: 'series', relativePath: 'Show/Show.S02E04.mkv' }),
  ]
  assert.deepEqual(
    filterItems(items, { kind: 'all', text: 'heat' }).map((entry) => entry.id),
    ['a'],
    'case does not matter',
  )
  assert.deepEqual(
    filterItems(items, { kind: 'all', text: 's02e04' }).map((entry) => entry.id),
    ['b'],
    'a filename search finds an episode the title does not name',
  )
  assert.deepEqual(
    filterItems(items, { kind: 'all', text: '   ' }).map((entry) => entry.id),
    ['a', 'b'],
    'whitespace is not a search',
  )
  assert.deepEqual(
    filterItems(items, { kind: 'movies', text: '' }).map((entry) => entry.id),
    ['a'],
  )
  assert.deepEqual(filterItems(items, NO_FILTER).length, 2)
})

test('an empty screen names the situation it is actually in', () => {
  // A first run, an unreadable folder, and a search with no hits are three different
  // situations with three different remedies.
  const firstRun = emptyState({
    sourceCount: 0,
    catalogCount: 0,
    scanning: false,
    unavailableSources: 0,
    filterText: '',
  })
  const unreadable = emptyState({
    sourceCount: 1,
    catalogCount: 0,
    scanning: false,
    unavailableSources: 1,
    filterText: '',
  })
  const filtered = emptyState({
    sourceCount: 1,
    catalogCount: 50,
    scanning: false,
    unavailableSources: 0,
    filterText: 'zzz',
  })

  assert.equal(firstRun?.situation, 'first_run')
  assert.equal(unreadable?.situation, 'source_unreadable')
  assert.equal(filtered?.situation, 'filtered')

  const situations = [firstRun, unreadable, filtered].map((state) => state?.situation)
  assert.equal(new Set(situations).size, 3, 'the three situations must not share copy')
  for (const state of [firstRun, unreadable, filtered]) {
    assert.ok((state?.title.length ?? 0) > 0)
    assert.ok(state?.detail.endsWith('.'), 'copy should be a sentence')
  }
})

test('an empty screen says nothing when there is something to show', () => {
  assert.equal(
    emptyState({
      sourceCount: 1,
      catalogCount: 10,
      scanning: false,
      unavailableSources: 0,
      filterText: '',
    }),
    null,
  )
})

test('a running scan explains itself before a filter does', () => {
  const scanning = emptyState({
    sourceCount: 1,
    catalogCount: 0,
    scanning: true,
    unavailableSources: 0,
    filterText: 'zzz',
  })
  assert.equal(scanning?.situation, 'scanning')
})

test('continue watching follows the core rule and orders by recency', () => {
  const items = [
    item({ id: 'old', workId: 'w-old', positionMs: 1000, updatedMs: 10 }),
    item({ id: 'new', workId: 'w-new', positionMs: 2000, updatedMs: 30 }),
    item({ id: 'middle', workId: 'w-mid', positionMs: 3000, updatedMs: 20 }),
    item({ id: 'unstarted', workId: 'w-u', positionMs: 0, updatedMs: 40 }),
    item({ id: 'finished', workId: 'w-f', positionMs: 5000, watched: true, updatedMs: 50 }),
    item({ id: 'hidden', workId: 'w-h', positionMs: 5000, hidden: true, updatedMs: 60 }),
    item({ id: 'gone', workId: 'w-g', positionMs: 5000, available: false, updatedMs: 70 }),
  ]
  assert.deepEqual(
    continueWatching(items, 10).map((entry) => entry.item.id),
    ['new', 'middle', 'old'],
  )
  assert.deepEqual(continueWatching(items, 2).map((entry) => entry.item.id), ['new', 'middle'])
  assert.deepEqual(
    continueWatching(items, 10).map((entry) => entry.positionMs),
    [2000, 3000, 1000],
    'the entry carries the position it was ordered by',
  )
})

test('two entries touched in the same millisecond keep a stable order', () => {
  const items = [
    item({ id: 'b', workId: 'w-b', positionMs: 1, updatedMs: 5 }),
    item({ id: 'a', workId: 'w-a', positionMs: 1, updatedMs: 5 }),
  ]
  assert.deepEqual(continueWatching(items, 5).map((entry) => entry.item.id), ['a', 'b'])
})

test('a position reads at a glance', () => {
  assert.equal(formatPosition(0), '0:00')
  assert.equal(formatPosition(64_000), '1:04')
  assert.equal(formatPosition(3_724_000), '1:02:04', 'hours appear only when there are any')
  assert.equal(formatPosition(-5), '0:00', 'a negative position is not shown as negative')
})

test('a progress fraction is bounded and tolerant of an unknown duration', () => {
  assert.equal(progressFraction(300_000, 600_000), 0.5)
  assert.equal(progressFraction(900_000, 600_000), 1)
  assert.equal(progressFraction(1000, 0), 0)
  assert.equal(progressFraction(Number.NaN, 600_000), 0)
})
