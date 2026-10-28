/**
 * The device's own playback preferences, as a pure value.
 *
 * There is no settings service and no account: these three fields are what the owner
 * told this television, and they are stored in the same document as the library so one
 * write covers both. That is the whole design, and it exists because a TV is used by
 * one or two people who should not have to re-pick their language every episode.
 *
 * What the app *learns* rather than asks: when the owner chooses a German audio track
 * in the player, German becomes the preference for the next file. That is a real
 * convenience (it is what every TV app does) and it also avoids inventing a settings
 * screen with a language list nobody can be bothered to scroll — a list this app cannot
 * populate honestly anyway, because it has no metadata provider and so does not know
 * which languages the library contains until a file has been opened.
 */

/** What to do about text tracks when a file opens. */
export type SubtitleMode = 'off' | 'auto'

/** The three preferences this device keeps. */
export interface DeviceSettings {
  /** `off` plays files with subtitles hidden; `auto` turns a matching track on. */
  readonly subtitles: SubtitleMode
  /** Preferred subtitle language, lowercase, or `''` for the file's own default. */
  readonly subtitleLanguage: string
  /** Preferred audio language, lowercase, or `''` to leave the decoder's choice alone. */
  readonly audioLanguage: string
}

/**
 * What a television starts with: subtitles off, both languages left to the file.
 *
 * Off rather than auto because turning subtitles *on* for someone who did not ask is
 * the more intrusive mistake, and the owner has already told the television what they
 * want the moment they touch the track menu.
 */
export const DEFAULT_SETTINGS: DeviceSettings = {
  subtitles: 'off',
  subtitleLanguage: '',
  audioLanguage: '',
}

/** A language code, normalised: trimmed, lowercased, and never a sentence. */
export function normaliseLanguage(value: unknown): string {
  if (typeof value !== 'string') {
    return ''
  }
  const trimmed = value.trim().toLowerCase()
  return trimmed.length <= 24 ? trimmed : ''
}

/**
 * Reads stored settings, or the defaults.
 *
 * Refuses rather than guesses: an unreadable mode is `off`, not "whatever it said", and
 * a language that is not a plausible code is dropped. A settings object is one of the
 * few things where the safe default is also the harmless one.
 */
export function decodeSettings(raw: unknown): DeviceSettings {
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) {
    return DEFAULT_SETTINGS
  }
  const record = raw as Record<string, unknown>
  const mode = record['subtitles'] === 'auto' ? 'auto' : 'off'
  return {
    subtitles: mode,
    subtitleLanguage: normaliseLanguage(record['subtitleLanguage']),
    audioLanguage: normaliseLanguage(record['audioLanguage']),
  }
}

/**
 * Remembers a language the owner chose, so the next file opens the way they left this one.
 *
 * The three cases are the three things the player's menu can do:
 *
 * * an audio track was chosen: that language becomes the audio preference;
 * * a text track was chosen: that language becomes the subtitle preference and
 *   subtitles turn *on*, because choosing a subtitle track is the clearest possible
 *   statement that they are wanted;
 * * subtitles were turned off: the mode flips to `off` and the language is kept, so
 *   turning them back on later restores the language rather than starting over.
 */
export function rememberChoice(
  settings: DeviceSettings,
  choice:
    | { readonly kind: 'audio'; readonly language: string | null }
    | { readonly kind: 'text'; readonly language: string | null }
    | { readonly kind: 'off' },
): DeviceSettings {
  switch (choice.kind) {
    case 'audio': {
      const language = normaliseLanguage(choice.language)
      return { ...settings, audioLanguage: language }
    }
    case 'text': {
      const language = normaliseLanguage(choice.language)
      return {
        ...settings,
        // A track with no declared language still counts as a choice: the mode turns on
        // and the language stays open, which is what "any" means here.
        subtitles: 'auto',
        subtitleLanguage: language,
      }
    }
    case 'off':
      return { ...settings, subtitles: 'off' }
  }
}

/** Clears the learned preferences, back to a television that has been told nothing. */
export function forgetChoices(): DeviceSettings {
  return DEFAULT_SETTINGS
}

/** The subtitle preference as a sentence, for the Settings screen. */
export function describeSubtitles(settings: DeviceSettings): string {
  if (settings.subtitles === 'off') {
    return 'Off — you can turn them on per file'
  }
  return settings.subtitleLanguage === ''
    ? 'On, in whichever language the file defaults to'
    : `On, preferring ${settings.subtitleLanguage}`
}

/** The audio preference as a sentence, for the Settings screen. */
export function describeAudio(settings: DeviceSettings): string {
  return settings.audioLanguage === ''
    ? 'Whatever the file defaults to'
    : `Preferring ${settings.audioLanguage}`
}
