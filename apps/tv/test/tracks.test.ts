import { test } from 'node:test'
import assert from 'node:assert/strict'

import type { LibraryItem } from '../src/core/contract.ts'
import {
  buildTrackMenu,
  chooseTracks,
  nextEpisode,
  selectableTracks,
  sidecarFileNames,
  SUBTITLE_SIDECAR_EXTENSIONS,
  type TrackChoice,
} from '../src/core/tracks.ts'

/** A decoder-reported track, with only the parts a test cares about spelled out. */
function track(type: string, index: number, language: string | null = null): TrackChoice {
  return {
    type,
    index,
    language,
    label: language === null ? `${type} ${String(index)}` : `${type} ${String(index)} · ${language}`,
  }
}

/** One catalogued episode, with the fields a next-episode test needs. */
function episode(
  id: string,
  season: number,
  number: number,
  overrides: Partial<LibraryItem> = {},
): LibraryItem {
  return {
    id,
    workId: 'work-series',
    title: `Episode ${String(number)}`,
    kind: 'series',
    season,
    episode: number,
    year: null,
    relativePath: `Show/S${String(season)}E${String(number)}.mkv`,
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

test('only the subtitle kinds the TV actually renders are offered beside a video', () => {
  // The four extensions are Samsung's own list. `.ass` and `.ssa` are deliberately
  // absent: offering a file the decoder refuses is worse than not offering it.
  assert.deepEqual([...SUBTITLE_SIDECAR_EXTENSIONS], ['smi', 'srt', 'sub', 'ttxt'])
  assert.deepEqual(sidecarFileNames('Show/S01E01'), [
    'Show/S01E01.smi',
    'Show/S01E01.srt',
    'Show/S01E01.sub',
    'Show/S01E01.ttxt',
  ])
  assert.equal(
    sidecarFileNames('Film').some((name) => name.endsWith('.ass')),
    false,
  )
})

test('the menu lists audio, then the subtitle switch, then the subtitle tracks', () => {
  const rows = buildTrackMenu(
    [track('VIDEO', 0), track('AUDIO', 1, 'eng'), track('AUDIO', 2, 'deu'), track('TEXT', 3, 'eng')],
    { audioIndex: 2, textIndex: null, silent: false },
  )
  assert.deepEqual(
    rows.map((row) => row.key),
    ['audio:1', 'audio:2', 'text:off', 'text:3'],
  )
  // The chosen audio track is the marked one; the un-chosen one is not.
  assert.equal(rows[1]?.selected, true)
  assert.equal(rows[0]?.selected, false)
  // Nothing has been chosen for text, and the decoder is not silenced, so "off" is the
  // truthful state: a file starts with no subtitle track picked.
  assert.equal(rows[2]?.selected, true)
  assert.equal(rows[3]?.selected, false)
})

test('a file with no subtitle tracks gets no "subtitles off" row', () => {
  const rows = buildTrackMenu([track('AUDIO', 0, 'eng')], {
    audioIndex: null,
    textIndex: null,
    silent: false,
  })
  assert.equal(rows.length, 1)
  assert.equal(rows[0]?.kind, 'audio')
  assert.equal(rows.some((row) => row.kind === 'off'), false)
})

test('a silenced decoder marks "off" even when a track is selected', () => {
  const rows = buildTrackMenu([track('AUDIO', 0), track('TEXT', 1, 'eng')], {
    audioIndex: 0,
    textIndex: 1,
    silent: true,
  })
  const off = rows.find((row) => row.kind === 'off')
  const text = rows.find((row) => row.kind === 'text')
  assert.equal(off?.selected, true)
  assert.equal(text?.selected, false, 'a silenced track is not offered as showing')
})

test('only audio and text are switchable', () => {
  const grouped = selectableTracks([
    track('VIDEO', 0),
    track('AUDIO', 1),
    track('TEXT', 2),
    track('SUBTITLE', 3),
  ])
  assert.deepEqual(
    grouped.audio.map((entry) => entry.index),
    [1],
  )
  assert.deepEqual(
    grouped.text.map((entry) => entry.index),
    [2],
  )
})

test('a stored preference picks the matching track, in either language spelling', () => {
  const tracks = [track('AUDIO', 0, 'eng'), track('AUDIO', 1, 'deu'), track('TEXT', 2, 'deu')]
  // `de` against `deu` is the spelling difference the rule exists for.
  const choice = chooseTracks(tracks, {
    subtitles: 'auto',
    subtitleLanguage: 'de',
    audioLanguage: 'deu',
  })
  assert.equal(choice.audioIndex, 1)
  assert.equal(choice.textIndex, 2)
  assert.equal(choice.silent, false)
  assert.equal(choice.selectAudio, true)
  assert.equal(choice.selectText, true)
})

test('an empty preference selects nothing at all', () => {
  // Leaving the decoder's own default is not the same as choosing a track for the
  // owner: a film must not open on the commentary because it was listed first.
  const choice = chooseTracks([track('AUDIO', 0, 'eng'), track('AUDIO', 1, 'eng')], {
    subtitles: 'off',
    subtitleLanguage: '',
    audioLanguage: '',
  })
  assert.equal(choice.audioIndex, null)
  assert.equal(choice.selectAudio, false)
  assert.equal(choice.selectText, false)
  assert.equal(choice.silent, true)
})

test('a language the file does not carry selects nothing rather than the first track', () => {
  const choice = chooseTracks([track('AUDIO', 0, 'eng'), track('AUDIO', 1, 'fra')], {
    subtitles: 'auto',
    subtitleLanguage: 'jpn',
    audioLanguage: 'deu',
  })
  assert.equal(choice.audioIndex, null)
  assert.equal(choice.textIndex, null)
  assert.equal(choice.selectText, false)
})

test('subtitles off is silence, not a track index', () => {
  const choice = chooseTracks([track('AUDIO', 0), track('TEXT', 1, 'eng')], {
    subtitles: 'off',
    subtitleLanguage: 'eng',
    audioLanguage: '',
  })
  assert.equal(choice.silent, true)
  assert.equal(choice.textIndex, null)
  assert.equal(choice.selectText, false)
})

test('the next episode is the one file exactly one number up, or nothing', () => {
  const library = [episode('e1', 1, 1), episode('e2', 1, 2), episode('e3', 1, 3)]
  assert.equal(nextEpisode(library, library[0]!)?.id, 'e2')
  assert.equal(nextEpisode(library, library[2]!), null, 'the last episode ends the run')
})

test('a gap in the numbering is never skipped over', () => {
  // Episode 2 is missing. Jumping to 3 is how somebody watches the second half of a
  // story without knowing they missed the first.
  const library = [episode('e1', 1, 1), episode('e3', 1, 3)]
  assert.equal(nextEpisode(library, library[0]!), null)
})

test('an unreadable or duplicated next episode is not offered', () => {
  const unavailable = [episode('e1', 1, 1), episode('e2', 1, 2, { available: false })]
  assert.equal(nextEpisode(unavailable, unavailable[0]!), null)
  const duplicated = [
    episode('e1', 1, 1),
    episode('e2', 1, 2),
    episode('e2b', 1, 2, { relativePath: 'Show/other.mkv' }),
  ]
  assert.equal(nextEpisode(duplicated, duplicated[0]!), null, 'two claimants is a coin toss')
})

test('a film, another season and another work are all refused', () => {
  const film = episode('f1', 1, 1, { kind: 'movie', season: null, episode: null })
  assert.equal(nextEpisode([film], film), null)
  const library = [
    episode('e1', 1, 4),
    episode('e2', 2, 5, { relativePath: 'Show/S02E05.mkv' }),
    episode('o1', 1, 5, { workId: 'other-work' }),
  ]
  assert.equal(nextEpisode(library, library[0]!), null, 'a season is not rolled over')
})
