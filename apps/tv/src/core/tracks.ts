/**
 * Tracks, subtitles and what comes next, as pure functions.
 *
 * Three decisions live here because all three are the kind that go wrong quietly:
 *
 * * **Which track a file opens with.** The owner's stored preference is applied by a
 *   rule, not by "the first track", so an episode does not open in a language nobody
 *   asked for just because the release put it first.
 * * **What "subtitles off" means.** It is not a track index: AVPlay hides text with
 *   `setSilentSubtitle(true)` and keeps the track selected, and an app that instead
 *   tried to deselect a track would leave subtitles on in the one case that matters.
 * * **Whether there *is* a next episode.** The Rust core refuses a guess here in the
 *   same terms: exactly one candidate one number up, or nothing. A gap must never be
 *   skipped, because skipping an episode is how somebody watches the second half of a
 *   story without knowing they missed the first.
 */

import type { LibraryItem } from './contract.ts'

/** A selectable track, as the app shows it. */
export interface TrackChoice {
  /** The AVPlay track type, kept as the platform spells it. */
  readonly type: string
  /** The index `setSelectTrack` takes. */
  readonly index: number
  /** The language from the decoder, or `null` when it did not say. */
  readonly language: string | null
  /** A label for the menu, never empty. */
  readonly label: string
}

/** The subtitle-file extensions Samsung's own player reads beside a video. */
export const SUBTITLE_SIDECAR_EXTENSIONS: readonly string[] = ['smi', 'srt', 'sub', 'ttxt']

/**
 * The sidecar names to look for beside a video.
 *
 * The four extensions are the ones Samsung documents for its own subtitle support
 * (SAMI, SubRip, SubViewer and MPEG-4 Timed Text). `.ass` and `.ssa` are deliberately
 * absent: Samsung's player does not render them, and offering a file the decoder will
 * refuse is worse than not offering it.
 */
export function sidecarFileNames(stem: string): string[] {
  return SUBTITLE_SIDECAR_EXTENSIONS.map((extension) => `${stem}.${extension}`)
}

/** The tracks the decoder offers, grouped the way the menu shows them. */
export function selectableTracks(tracks: readonly TrackChoice[]): {
  readonly audio: readonly TrackChoice[]
  readonly text: readonly TrackChoice[]
} {
  return {
    audio: tracks.filter((track) => track.type.toUpperCase() === 'AUDIO'),
    text: tracks.filter((track) => track.type.toUpperCase() === 'TEXT'),
  }
}

/** What the player currently believes is selected. */
export interface TrackState {
  /** The selected audio track index, or `null` when the decoder's default stands. */
  readonly audioIndex: number | null
  /** The selected text track index, or `null` when none was chosen explicitly. */
  readonly textIndex: number | null
  /** True when the decoder is rendering text silently. */
  readonly silent: boolean
}

/** One row of the track menu. */
export interface TrackMenuRow {
  /** A stable key for the renderer. */
  readonly key: string
  /** What choosing this row does. */
  readonly kind: 'audio' | 'text' | 'off'
  /** The tracks the row names, empty for the off row. */
  readonly track: TrackChoice | null
  readonly label: string
  readonly detail: string
  readonly selected: boolean
}

/**
 * Builds the menu for the tracks the decoder reported.
 *
 * One flat list, in the order a person thinks about it: audio first, because that is
 * what a file always has; then the subtitle switch, then the subtitle tracks. A flat
 * list is also what the focus machine already handles — the same flat index the grids
 * use — so the menu needs no second navigation model and no second set of rules.
 *
 * A file with no text tracks gets no off row: a control that switches off something
 * that was never on is a control that does nothing.
 */
export function buildTrackMenu(
  tracks: readonly TrackChoice[],
  state: TrackState,
): TrackMenuRow[] {
  const grouped = selectableTracks(tracks)
  const rows: TrackMenuRow[] = []

  for (const track of grouped.audio) {
    const language = track.language?.trim() ?? ''
    rows.push({
      key: `audio:${String(track.index)}`,
      kind: 'audio',
      track,
      label: track.label,
      detail: language === '' ? 'No language declared by the decoder' : `Audio in ${language}`,
      selected: state.audioIndex !== null && state.audioIndex === track.index,
    })
  }

  if (grouped.text.length > 0) {
    rows.push({
      key: 'text:off',
      kind: 'off',
      track: null,
      label: 'Subtitles off',
      detail: 'Hides subtitles without changing the file.',
      // Off is selected when the decoder is silenced *or* when nothing was chosen,
      // which is the state a file starts in.
      selected: state.silent || state.textIndex === null,
    })
    for (const track of grouped.text) {
      const language = track.language?.trim() ?? ''
      rows.push({
        key: `text:${String(track.index)}`,
        kind: 'text',
        track,
        label: track.label,
        detail: language === '' ? 'No language declared by the decoder' : `Subtitles in ${language}`,
        selected: !state.silent && state.textIndex === track.index,
      })
    }
  }

  return rows
}

/** The preferences a file is opened with. */
export interface TrackPreference {
  /** `off` keeps text hidden; `auto` looks for a matching text track. */
  readonly subtitles: 'off' | 'auto'
  /** Preferred subtitle language, lowercase, or `''` for the file's own default. */
  readonly subtitleLanguage: string
  /** Preferred audio language, lowercase, or `''` to leave the decoder's choice. */
  readonly audioLanguage: string
}

/** True when a track's declared language matches a preference. */
function matches(track: TrackChoice, language: string): boolean {
  if (language === '') {
    return true
  }
  const declared = track.language?.trim().toLowerCase() ?? ''
  if (declared === '') {
    return false
  }
  // Codes are spelled two ways in the wild — `de` and `deu`, `en` and `eng` — so a
  // prefix match either way is accepted rather than a strict equality that would miss
  // half a library.
  return declared === language || declared.startsWith(language) || language.startsWith(declared)
}

/**
 * What to select when a file opens.
 *
 * Returns indices rather than making the calls, so the choice has a test and the
 * platform adapter stays a mapping. `null` means "do not call `setSelectTrack`", which
 * is deliberately *not* the same as giving the decoder a default: issuing a selection
 * the owner did not ask for is how a film ends up in the commentary track.
 */
export function chooseTracks(
  tracks: readonly TrackChoice[],
  preference: TrackPreference,
): TrackState & { readonly selectAudio: boolean; readonly selectText: boolean } {
  const grouped = selectableTracks(tracks)

  let audioIndex: number | null = null
  if (preference.audioLanguage !== '') {
    const audio = grouped.audio.find((track) => matches(track, preference.audioLanguage))
    audioIndex = audio?.index ?? null
  }

  if (preference.subtitles === 'off') {
    return { audioIndex, textIndex: null, silent: true, selectAudio: audioIndex !== null, selectText: false }
  }

  const text =
    preference.subtitleLanguage === ''
      ? grouped.text[0]
      : grouped.text.find((track) => matches(track, preference.subtitleLanguage))
  if (text === undefined) {
    // Asked for subtitles and the file has none: silent, and honest about it in the
    // menu rather than pretending a track is showing.
    return { audioIndex, textIndex: null, silent: true, selectAudio: audioIndex !== null, selectText: false }
  }
  return {
    audioIndex,
    textIndex: text.index,
    silent: false,
    selectAudio: audioIndex !== null,
    selectText: true,
  }
}

/**
 * The verified next episode, or `null`.
 *
 * The Rust core's same-season rule, and its refusal to roll a season over: this client
 * has no verified episode ordering of its own, and inventing one from filenames would
 * skip a missing episode the first time the naming drifted. So an episode at the end of
 * what is catalogued ends there, and the screen says so instead of guessing.
 */
export function nextEpisode(
  items: readonly LibraryItem[],
  current: LibraryItem,
): LibraryItem | null {
  if (current.kind !== 'series' || current.season === null || current.episode === null) {
    return null
  }
  const candidates = items.filter(
    (item) =>
      item.id !== current.id &&
      item.workId === current.workId &&
      item.season === current.season &&
      item.episode === current.episode! + 1 &&
      item.available,
  )
  // Exactly one, or nothing: two rows claiming episode 5 is a naming collision, and
  // picking one of them would be a coin toss.
  return candidates.length === 1 ? (candidates[0] ?? null) : null
}
