import { test } from 'node:test'
import assert from 'node:assert/strict'

import { candidateRoots, checkLocator, resolveLocator } from '../src/core/locator.ts'

test('a plain relative locator is accepted and split into segments', () => {
  const verdict = checkLocator('Shows/Season 1/Show.S01E01.mkv')
  assert.equal(verdict.safe, true)
  assert.deepEqual(verdict.safe ? verdict.segments : [], [
    'Shows',
    'Season 1',
    'Show.S01E01.mkv',
  ])
})

test('an absolute locator is refused on both separator conventions', () => {
  for (const locator of ['/etc/passwd', '\\Windows\\System32', '//server/share']) {
    const verdict = checkLocator(locator)
    assert.equal(verdict.safe, false, `${locator} must be refused`)
  }
})

test('a parent-directory segment is refused however it is spelled', () => {
  for (const locator of ['../outside.mkv', 'Shows/../../outside.mkv', 'Shows/..', '..\\ou.mkv']) {
    const verdict = checkLocator(locator)
    assert.equal(verdict.safe, false, `${locator} must be refused`)
    assert.equal(verdict.safe === false ? verdict.fault : '', 'parent_directory')
  }
})

test('a Windows drive prefix is refused even though this device is not Windows', () => {
  // The library can have been built on the Windows desktop and carried across, and a
  // stored string is not allowed to reach another drive just because it says so.
  for (const locator of ['C:/Media/Film.mkv', 'D:Film.mkv', 'c:\\Film.mkv']) {
    const verdict = checkLocator(locator)
    assert.equal(verdict.safe, false, `${locator} must be refused`)
    assert.equal(verdict.safe === false ? verdict.fault : '', 'windows_prefix')
  }
})

test('a NUL byte and an empty locator are refused', () => {
  assert.equal(checkLocator('Fil\0m.mkv').safe, false)
  assert.equal(checkLocator('').safe, false)
  assert.equal(checkLocator('./').safe, false)
})

test('a doubled separator is a spelling difference, not an escape', () => {
  const verdict = checkLocator('Shows//Film.mkv')
  assert.equal(verdict.safe, true)
  assert.deepEqual(verdict.safe ? verdict.segments : [], ['Shows', 'Film.mkv'])
})

test('joining keeps the file inside the granted root', () => {
  assert.equal(resolveLocator('/USB/Media', 'Shows/Film.mkv'), '/USB/Media/Shows/Film.mkv')
  // A trailing separator on the root is tolerated.
  assert.equal(resolveLocator('/USB/Media/', 'Film.mkv'), '/USB/Media/Film.mkv')
  // And an unsafe locator produces nothing to open, rather than an unvalidated path.
  assert.equal(resolveLocator('/USB/Media', '../Film.mkv'), null)
  assert.equal(resolveLocator('/USB/Media', '/etc/passwd'), null)
  // A relative root would resolve against the app's own storage, which is not where
  // media lives.
  assert.equal(resolveLocator('USB/Media', 'Film.mkv'), null)
})

test('a device path is normalised and a missing one yields nothing to guess with', () => {
  assert.deepEqual(candidateRoots('/USB/Media/'), ['/USB/Media'])
  assert.deepEqual(candidateRoots('USB/Media'), ['/USB/Media'])
  assert.deepEqual(candidateRoots(null), [])
  assert.deepEqual(candidateRoots('/'), [])
})
