import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  extensionOf,
  fileStem,
  folderHintOf,
  identityKeyOf,
  parseFilename,
} from '../src/core/name.ts'

/**
 * The first nine cases below are the Rust core's own tests, restated here.
 *
 * That is the point of them: the two clients must read a filename identically, or the
 * same drive shows a different library depending on which screen is attached to it.
 * If a rule changes in `crates/screenroom-core/src/filename.rs`, one of these should
 * fail rather than the two implementations drifting quietly apart.
 */

test('reads common episode names', () => {
  const parsed = parseFilename('Show.Name.S01E02.1080p.WEB-DL.mkv')
  assert.equal(parsed.kind, 'series')
  assert.equal(parsed.title, 'Show Name')
  assert.equal(parsed.season, 1)
  assert.equal(parsed.episode, 2)
  assert.equal(parsed.episodeEnd, 2)
  assert.ok(parsed.confidence >= 0.85)
})

test('reads alternate episode notation', () => {
  const parsed = parseFilename('Another.Show 2x07.mkv')
  assert.equal(parsed.kind, 'series')
  assert.equal(parsed.season, 2)
  assert.equal(parsed.episode, 7)
  assert.equal(parsed.title, 'Another Show')
})

test('keeps a combined episode range', () => {
  const parsed = parseFilename('Show_S01E01E02.mkv')
  assert.equal(parsed.season, 1)
  assert.equal(parsed.episode, 1)
  assert.equal(parsed.episodeEnd, 2)
})

test('rejects an absurd episode range', () => {
  // A range of 100+ episodes is a misread, so the file keeps episode 1 only.
  const parsed = parseFilename('Show.S01E01E150.mkv')
  assert.equal(parsed.episode, 1)
  assert.equal(parsed.episodeEnd, 1)
})

test('reads a year for movies', () => {
  const parsed = parseFilename('A.Movie.2021.2160p.BluRay.x265.mkv')
  assert.equal(parsed.kind, 'movie')
  assert.equal(parsed.title, 'A Movie')
  assert.equal(parsed.year, 2021)
  assert.equal(parsed.confidence, 0.75)
})

test('a year before the season marker truncates the title', () => {
  const parsed = parseFilename('Show.2019.S01E04.720p.mkv')
  assert.equal(parsed.year, 2019)
  assert.equal(parsed.title, 'Show')
  assert.equal(parsed.season, 1)
  assert.equal(parsed.episode, 4)
})

test('a year after the episode marker is release noise, not the title end', () => {
  // The Rust rule: a year only cuts the title when it comes first. `Show.S01E04.2021`
  // is an episode released in 2021, and truncating at the year would leave "Show S01E04".
  const parsed = parseFilename('Show.S01E04.2021.1080p.mkv')
  assert.equal(parsed.title, 'Show')
  assert.equal(parsed.season, 1)
  assert.equal(parsed.episode, 4)
})

test('numeric names stay unknown and playable', () => {
  const parsed = parseFilename('12345.mkv')
  assert.equal(parsed.kind, 'other')
  assert.equal(parsed.title, '12345')
  assert.equal(parsed.confidence, 0.1)
})

test('an empty name falls back to the folder', () => {
  const parsed = parseFilename('2160p.mkv', 'Features/The Film')
  assert.equal(parsed.kind, 'movie')
  assert.equal(parsed.title, 'The Film')
})

test('a season folder does not become a title', () => {
  const parsed = parseFilename('1080p.mkv', 'Show/Season 2')
  assert.equal(parsed.kind, 'other')
  assert.equal(parsed.title, '1080p')
})

test('the file stem keeps a leading-dot name intact', () => {
  // `Path::file_stem` treats `.hidden` as a name, not as an extension-less empty
  // string, and a port that split on the first dot would turn it into "".
  assert.equal(fileStem('.hidden'), '.hidden')
  assert.equal(fileStem('a.tar.gz'), 'a.tar')
  assert.equal(fileStem('noext'), 'noext')
  assert.equal(fileStem('folder/name.mkv'), 'name')
})

test('extension reading ignores a leading dot and lowercases', () => {
  assert.equal(extensionOf('Film.MKV'), 'mkv')
  assert.equal(extensionOf('.hidden'), null)
  assert.equal(extensionOf('noext'), null)
})

test('the folder hint drops a season folder', () => {
  assert.equal(folderHintOf('Show/Season 2/1080p.mkv'), 'Show')
  assert.equal(folderHintOf('Show/Film.mkv'), 'Show')
  assert.equal(folderHintOf('Film.mkv'), '')
})

test('a confident series joins one work, and its year is part of the key', () => {
  const source = 'source-removable1'
  const one = parseFilename('Show.Name.S01E02.mkv')
  const two = parseFilename('Show.Name.S01E03.mkv')
  assert.equal(identityKeyOf(one, source, 'a'), identityKeyOf(two, source, 'a'))
  const other = parseFilename('Show.Name.2019.S01E02.mkv')
  assert.notEqual(identityKeyOf(one, source, 'a'), identityKeyOf(other, source, 'a'))
})

test('a confident movie joins one work', () => {
  const source = 'source-removable1'
  const one = parseFilename('A.Movie.2021.2160p.mkv')
  const two = parseFilename('A Movie 2021.mkv')
  assert.equal(identityKeyOf(one, source, 'a'), identityKeyOf(two, source, 'a'))
  // A different year is a different work — the remake keeps its own progress.
  assert.notEqual(
    identityKeyOf(one, source, 'a'),
    identityKeyOf(parseFilename('A.Movie.1994.mkv'), source, 'a'),
  )
})

test('an unreadable name never joins another file to itself', () => {
  // Two files whose names cannot be read must not become one work. The unique suffix
  // is what stops a folder of camera dumps collapsing into a single film.
  const source = 'source-removable1'
  const one = parseFilename('12345.mkv')
  const two = parseFilename('67890.mkv')
  assert.equal(one.kind, 'other')
  assert.equal(two.kind, 'other')
  assert.notEqual(
    identityKeyOf(one, source, 'suffix-one'),
    identityKeyOf(two, source, 'suffix-two'),
  )
})

test('a series key is per source, so two drives do not share one work', () => {
  const parsed = parseFilename('Show.Name.S01E02.mkv')
  assert.notEqual(
    identityKeyOf(parsed, 'source-removable1', 'a'),
    identityKeyOf(parsed, 'source-removable2', 'a'),
  )
})
