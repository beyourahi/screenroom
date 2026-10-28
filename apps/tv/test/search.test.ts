import { test } from 'node:test'
import assert from 'node:assert/strict'

import type { LibraryItem } from '../src/core/contract.ts'
import {
  describeSearch,
  QUERY_MAX,
  SEARCH_KEYS,
  searchResults,
  withKey,
  type SearchKey,
} from '../src/core/search.ts'

function item(overrides: Partial<LibraryItem> = {}): LibraryItem {
  return {
    id: 'item-1',
    workId: 'work-1',
    title: 'Heat',
    kind: 'movie',
    season: null,
    episode: null,
    year: 1995,
    relativePath: 'Movies/Heat.1995.mkv',
    sourceId: 'source-1',
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

function key(id: string): SearchKey {
  const found = SEARCH_KEYS.find((candidate) => candidate.id === id)
  assert.ok(found !== undefined, `the keyboard has no key ${id}`)
  return found
}

test('the keyboard offers every letter and digit, plus the two editing keys', () => {
  const letters = SEARCH_KEYS.filter((entry) => entry.kind === 'char' && /^[A-Z]$/.test(entry.value))
  const digits = SEARCH_KEYS.filter((entry) => entry.kind === 'char' && /^[0-9]$/.test(entry.value))
  assert.equal(letters.length, 26)
  assert.equal(digits.length, 10)
  assert.deepEqual(
    SEARCH_KEYS.filter((entry) => entry.kind !== 'char').map((entry) => entry.kind),
    ['delete', 'clear'],
  )
  assert.equal(new Set(SEARCH_KEYS.map((entry) => entry.id)).size, SEARCH_KEYS.length)
})

test('a key press appends, deletes or clears the query', () => {
  let query = ''
  query = withKey(query, key('key-h'))
  query = withKey(query, key('key-e'))
  query = withKey(query, key('key-a'))
  query = withKey(query, key('key-t'))
  assert.equal(query, 'HEAT')
  assert.equal(withKey(query, key('space')), 'HEAT ')
  assert.equal(withKey('HEAT', key('delete')), 'HEA')
  assert.equal(withKey('HEAT', key('clear')), '')
  assert.equal(withKey('', key('delete')), '', 'deleting an empty query is not an error')
})

test('a held key cannot grow the query without bound', () => {
  const long = 'A'.repeat(QUERY_MAX)
  assert.equal(withKey(long, key('key-b')), long, 'a full query stops accepting characters')
  assert.equal(withKey('A'.repeat(QUERY_MAX - 1), key('key-b')).length, QUERY_MAX)
})

test('an empty query matches nothing rather than the whole library', () => {
  const items = [item(), item({ id: 'item-2', title: 'Arrival' })]
  assert.deepEqual(searchResults(items, ''), [])
  assert.deepEqual(searchResults(items, '   '), [])
})

test('a query finds a title, a show name or a file name, in display order', () => {
  const items = [
    item({ id: 'heat', title: 'Heat', relativePath: 'Movies/Heat.1995.mkv' }),
    item({ id: 'severance', title: 'Severance', kind: 'series', relativePath: 'Shows/Severance.S01E01.mkv', season: 1, episode: 1 }),
    item({ id: 'arrival', title: 'Arrival', relativePath: 'Movies/Arrival.2016.mkv' }),
  ]
  assert.deepEqual(searchResults(items, 'heat').map((entry) => entry.id), ['heat'])
  assert.deepEqual(
    searchResults(items, 's01e01').map((entry) => entry.id),
    ['severance'],
    'the filename is searchable, because that is what the owner may know',
  )
  assert.deepEqual(
    searchResults(items, 'arr').map((entry) => entry.id),
    ['arrival'],
    'a title match finds the film whose name contains it',
  )
  assert.deepEqual(
    searchResults(items, 's').map((entry) => entry.id),
    ['arrival', 'heat', 'severance'],
    'results are sorted the same way the library is',
  )
})

test('the status line says what was found, and what to do when nothing was', () => {
  assert.match(describeSearch('', 0), /Type a film/)
  assert.match(describeSearch('zzz', 0), /Nothing matches/)
  assert.match(describeSearch('heat', 1), /1 match for/)
  assert.match(describeSearch('a', 12), /12 matches for/)
})
