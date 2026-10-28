/**
 * The two pieces of AVPlay handling that are arithmetic, kept away from the platform.
 *
 * Both of these are places where the published samples are wrong, which is exactly why
 * they are pure functions with tests rather than a few lines buried in a callback:
 *
 * 1. **`setDisplayRect` is in a fixed 1920×1080 space.** The TV scales it, whatever
 *    the app's own resolution is, so a rect computed in app pixels lands in the wrong
 *    place — or off-screen entirely — unless it is converted first.
 * 2. **`getTotalTrackInfo()` needs per-entry reading.** The guide's own sample reads
 *    `totalTrackInfo.type` where it means `totalTrackInfo[i].type`, so it inspects
 *    the array rather than the track, finds nothing, and reports an empty menu.
 *
 * Nothing here touches the platform, so both rules are verifiable on a laptop.
 */

/** A rectangle in the app's own coordinate space. */
export interface Rect {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

/** The viewport AVPlay measures against: always 1920×1080, on every TV. */
export const AVPLAY_SPACE_WIDTH = 1920
export const AVPLAY_SPACE_HEIGHT = 1080

/** The app's own drawing surface, as the host reports it. */
export interface Viewport {
  readonly width: number
  readonly height: number
}

/**
 * Converts an app-space rectangle into AVPlay's 1920×1080 space.
 *
 * The conversion is uniform on both axes, because the app is authored at 1920×1080
 * and the host scales the whole surface: scaling x and y by different factors would
 * be right only if the app were letterboxed, which it is not.
 *
 * A degenerate viewport (zero or non-finite) falls back to the app's authored size,
 * so a measurement taken before layout cannot produce a rect of `NaN` or `Infinity`
 * and hand it to the decoder.
 */
export function toDisplayRect(rect: Rect, viewport: Viewport): Rect {
  const usableWidth =
    Number.isFinite(viewport.width) && viewport.width > 0 ? viewport.width : AVPLAY_SPACE_WIDTH
  const usableHeight =
    Number.isFinite(viewport.height) && viewport.height > 0 ? viewport.height : AVPLAY_SPACE_HEIGHT
  const scaleX = AVPLAY_SPACE_WIDTH / usableWidth
  const scaleY = AVPLAY_SPACE_HEIGHT / usableHeight
  const round = (value: number, scale: number): number => {
    const scaled = value * scale
    return Number.isFinite(scaled) ? Math.round(scaled) : 0
  }
  return {
    x: round(rect.x, scaleX),
    y: round(rect.y, scaleY),
    width: Math.max(0, round(rect.width, scaleX)),
    height: Math.max(0, round(rect.height, scaleY)),
  }
}

/** A selectable track, as the app shows it. */
export interface TrackChoice {
  /** The AVPlay track type, kept as the platform spells it. */
  readonly type: string
  /** The index `setSelectTrack` takes. */
  readonly index: number
  /** The language from `extra_info`, or `null` when the platform did not say. */
  readonly language: string | null
  /** A label for the menu, never empty. */
  readonly label: string
}

/** The track types this app offers a menu for. */
export const SELECTABLE_TRACK_TYPES = ['AUDIO', 'TEXT'] as const

/**
 * Reads `getTotalTrackInfo()` into the app's own shape.
 *
 * Entries are read one at a time, and an entry that is not a usable object with a
 * numeric index is skipped rather than crashing the menu — the array comes straight
 * from a decoder, and a malformed entry in it must not take the player down.
 *
 * `extra_info` is a JSON *string* and is documented as optional, so it is parsed
 * defensively: an unparseable string leaves the language unknown, which the label
 * then says honestly instead of inventing one.
 */
export function readTracks(raw: unknown): TrackChoice[] {
  if (!Array.isArray(raw)) {
    return []
  }
  const tracks: TrackChoice[] = []
  for (const entry of raw) {
    if (typeof entry !== 'object' || entry === null) {
      continue
    }
    const candidate = entry as { type?: unknown; index?: unknown; extra_info?: unknown }
    if (typeof candidate.type !== 'string' || !Number.isSafeInteger(candidate.index)) {
      continue
    }
    const index = candidate.index as number
    const language = readLanguage(candidate.extra_info)
    const type = candidate.type.toUpperCase()
    tracks.push({
      type,
      index,
      language,
      label: language === null ? `${type} ${index}` : `${type} ${index} · ${language}`,
    })
  }
  return tracks
}

/** The language out of an `extra_info` string, or `null` when it cannot be read. */
function readLanguage(extraInfo: unknown): string | null {
  if (typeof extraInfo !== 'string' || extraInfo.length === 0) {
    return null
  }
  try {
    const parsed: unknown = JSON.parse(extraInfo)
    if (typeof parsed !== 'object' || parsed === null) {
      return null
    }
    const language = (parsed as { language?: unknown; Language?: unknown }).language
    if (typeof language === 'string' && language.length > 0) {
      return language
    }
    const capitalised = (parsed as { Language?: unknown }).Language
    return typeof capitalised === 'string' && capitalised.length > 0 ? capitalised : null
  } catch {
    return null
  }
}

/**
 * Groups tracks into the menus the player screen offers.
 *
 * Audio and text are the two the app can actually switch, so those are the two it
 * lists; anything else the decoder reports is left alone rather than shown as a
 * control that would do nothing.
 */
export function selectableTracks(tracks: readonly TrackChoice[]): {
  readonly audio: readonly TrackChoice[]
  readonly text: readonly TrackChoice[]
} {
  return {
    audio: tracks.filter((track) => track.type === 'AUDIO'),
    text: tracks.filter((track) => track.type === 'TEXT'),
  }
}
