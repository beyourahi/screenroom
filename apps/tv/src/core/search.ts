/**
 * Search: the on-screen keyboard, and what a query matches.
 *
 * A television has no text field to type into, so search has to be a grid of keys the
 * D-pad can reach. That is the whole reason this is a module with tests rather than a
 * `<input>`: the query is built one key press at a time, the keys have to stay in a
 * fixed order, and a viewer has to be able to fix a typo with the same four arrow keys
 * they used to make it.
 *
 * Two rules are worth stating:
 *
 * * **An empty query matches nothing.** Returning the whole library for `''` would draw
 *   a thousand rows behind an empty search box and make the screen look identical to
 *   Movies. Search is empty until something is typed.
 * * **The query is bounded.** A remote held down on one key must not grow a string
 *   without limit, so a full query simply stops accepting characters.
 */

import { filterItems, sortForDisplay } from './browse.ts'
import type { LibraryItem } from './contract.ts'

/** The longest query this screen will build, in characters. */
export const QUERY_MAX = 40

/** One key on the on-screen keyboard. */
export interface SearchKey {
  /** Stable key for the renderer. */
  readonly id: string
  /** What the key draws. */
  readonly label: string
  readonly kind: 'char' | 'delete' | 'clear'
  /** What a character key appends. Empty for the two editing keys. */
  readonly value: string
}

/** The keyboard, in reading order: letters, then digits, then the editing keys. */
export const SEARCH_KEYS: readonly SearchKey[] = [
  ...'ABCDEF'.split('').map(charKey),
  ...'GHIJKL'.split('').map(charKey),
  ...'MNOPQR'.split('').map(charKey),
  ...'STUVWX'.split('').map(charKey),
  ...'YZ0123'.split('').map(charKey),
  ...'456789'.split('').map(charKey),
  { id: 'space', label: 'Space', kind: 'char', value: ' ' },
  { id: 'delete', label: '⌫', kind: 'delete', value: '' },
  { id: 'clear', label: 'Clear', kind: 'clear', value: '' },
]

/** A character key, as one line so the table above stays readable. */
function charKey(char: string): SearchKey {
  return { id: `key-${char.toLowerCase()}`, label: char, kind: 'char', value: char }
}

/** The query after one key press. */
export function withKey(query: string, key: SearchKey): string {
  switch (key.kind) {
    case 'char':
      return query.length >= QUERY_MAX ? query : `${query}${key.value}`
    case 'delete':
      return query.slice(0, -1)
    case 'clear':
      return ''
  }
}

/**
 * What a query matches.
 *
 * Matching is `filterItems` from `core/browse.ts`, so a search finds a title, a show
 * name or a file name exactly the same way the library's own text filter does. An empty
 * (or whitespace-only) query has no results at all.
 */
export function searchResults(
  items: readonly LibraryItem[],
  query: string,
): LibraryItem[] {
  const trimmed = query.trim()
  if (trimmed.length === 0) {
    return []
  }
  return sortForDisplay(filterItems(items, { kind: 'all', text: trimmed }))
}

/** The line above the results: what is being searched for, and how much was found. */
export function describeSearch(query: string, matches: number): string {
  if (query.trim().length === 0) {
    return 'Type a film, a show or a file name with the arrow keys and OK.'
  }
  if (matches === 0) {
    return `Nothing matches “${query.trim()}”. ⌫ removes a letter, Clear starts again.`
  }
  return `${matches} ${matches === 1 ? 'match' : 'matches'} for “${query.trim()}”.`
}
