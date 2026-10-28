import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  decodeSettings,
  DEFAULT_SETTINGS,
  describeAudio,
  describeSubtitles,
  forgetChoices,
  normaliseLanguage,
  rememberChoice,
  type DeviceSettings,
} from '../src/core/settings.ts'

test('a television that has been told nothing starts with subtitles off', () => {
  // Off rather than auto: turning subtitles on for somebody who did not ask is the more
  // intrusive mistake of the two.
  assert.equal(DEFAULT_SETTINGS.subtitles, 'off')
  assert.equal(DEFAULT_SETTINGS.subtitleLanguage, '')
  assert.equal(DEFAULT_SETTINGS.audioLanguage, '')
})

test('stored settings are read, and anything unreadable falls back to the default', () => {
  assert.deepEqual(decodeSettings(undefined), DEFAULT_SETTINGS)
  assert.deepEqual(decodeSettings(null), DEFAULT_SETTINGS)
  assert.deepEqual(decodeSettings([]), DEFAULT_SETTINGS)
  assert.deepEqual(decodeSettings('auto'), DEFAULT_SETTINGS)
  assert.deepEqual(decodeSettings({ subtitles: 'sometimes' }), DEFAULT_SETTINGS)
  assert.deepEqual(
    decodeSettings({ subtitles: 'auto', subtitleLanguage: 'DE', audioLanguage: 'deu' }),
    { subtitles: 'auto', subtitleLanguage: 'de', audioLanguage: 'deu' },
  )
})

test('a language code is normalised and never stored as a sentence', () => {
  assert.equal(normaliseLanguage('  ENG '), 'eng')
  assert.equal(normaliseLanguage(undefined), '')
  assert.equal(normaliseLanguage(42), '')
  assert.equal(normaliseLanguage('x'.repeat(40)), '', 'a paragraph is not a language code')
})

test('choosing an audio track remembers its language', () => {
  const next = rememberChoice(DEFAULT_SETTINGS, { kind: 'audio', language: 'DEU' })
  assert.equal(next.audioLanguage, 'deu')
  // An audio choice says nothing about subtitles.
  assert.equal(next.subtitles, 'off')
})

test('choosing a subtitle track turns subtitles on and remembers the language', () => {
  const next = rememberChoice(DEFAULT_SETTINGS, { kind: 'text', language: 'eng' })
  assert.equal(next.subtitles, 'auto')
  assert.equal(next.subtitleLanguage, 'eng')
})

test('turning subtitles off keeps the language, so turning them back on restores it', () => {
  const withSubtitles: DeviceSettings = {
    subtitles: 'auto',
    subtitleLanguage: 'fra',
    audioLanguage: 'eng',
  }
  const off = rememberChoice(withSubtitles, { kind: 'off' })
  assert.equal(off.subtitles, 'off')
  assert.equal(off.subtitleLanguage, 'fra')
  assert.equal(off.audioLanguage, 'eng')
})

test('a track with no declared language still counts as a choice', () => {
  const next = rememberChoice(DEFAULT_SETTINGS, { kind: 'text', language: null })
  assert.equal(next.subtitles, 'auto')
  assert.equal(next.subtitleLanguage, '')
})

test('the settings screen describes each state in a sentence', () => {
  assert.match(describeSubtitles(DEFAULT_SETTINGS), /^Off/)
  assert.match(describeSubtitles({ ...DEFAULT_SETTINGS, subtitles: 'auto' }), /^On, in whichever language/)
  assert.match(
    describeSubtitles({ ...DEFAULT_SETTINGS, subtitles: 'auto', subtitleLanguage: 'deu' }),
    /preferring deu/,
  )
  assert.match(describeAudio(DEFAULT_SETTINGS), /whatever the file defaults to/i)
  assert.match(
    describeAudio({ ...DEFAULT_SETTINGS, audioLanguage: 'eng' }),
    /Preferring eng/,
  )
})

test('forgetting the learned choices returns the television to its defaults', () => {
  assert.deepEqual(forgetChoices(), DEFAULT_SETTINGS)
})
