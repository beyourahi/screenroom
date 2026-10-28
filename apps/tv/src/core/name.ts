/**
 * Filename interpretation, ported from the Rust core.
 *
 * This is a *hint*, not an identity — the same sentence
 * `crates/screenroom-core/src/filename.rs` opens with. A name that cannot be read
 * confidently stays `other` and remains playable, because a wrong guess is worse than
 * an honest unknown: it would file one film under another's title and offer the wrong
 * artwork, the wrong resume point and the wrong episode.
 *
 * Every rule below is the Rust rule, regex for regex and cut for cut, so the two
 * devices group the same folder into the same works. The one thing that is *not* a
 * port is what the result is used for: the TV has no metadata provider, so the parsed
 * title is only ever displayed, never looked up.
 *
 * The order of the two "cut" candidates is the subtle part. A year is only treated as
 * the end of the title when it appears *before* the episode marker — `Show.2019.S01E04`
 * ends its title at the year, while `Show.S01E04.2019` must not, because that year is
 * part of the release tag rather than the title.
 */

import type { MediaKind } from './contract.ts'

/** An episodic marker: `S01E02`, `S01E02E03`, `s1e2`, `S01 E02`, and the like. */
const EPISODIC_SE = /\bS(\d{1,3})[ ._-]*E(\d{1,4})(?:[ ._-]*(?:E|-E?)(\d{1,4}))?/i

/** The other episodic spelling: `2x07`, `2x07-08`. */
const EPISODIC_AX = /\b(\d{1,3})x(\d{1,4})(?:-(\d{1,4}))?/i

/** A four-digit year in the plausible range. */
const YEAR = /\b(19\d{2}|20\d{2})\b/

/** Release quality tags. Their first appearance ends the title. */
const QUALITY =
  /\b(?:2160p|1080p|720p|480p|bluray|brrip|webrip|web-dl|hdtv|dvdrip|x26[45]|h26[45]|hevc|av1)\b/i

/** Brackets are release noise, never part of a title. */
const BRACKETS = /[[\]()]+/

/** Dots and underscores separate words. */
const SEPARATORS = /[._]/g

/** A folder that names its season, which must not become a title. */
const SEASON_FOLDER = /^season\s*\d+$/i

/** A name that is only digits carries no title. */
const NUMERIC_TITLE = /^\d+$/

/** What a filename suggests about the media it holds. */
export interface ParsedName {
  /** Best-effort title, or the file stem when nothing could be read. */
  readonly title: string
  /** Interpreted shape. `other` is this client's honest "not sure". */
  readonly kind: MediaKind
  /** Release year, when it appears before any quality tag. */
  readonly year: number | null
  /** Season number, for episodic names. */
  readonly season: number | null
  /** First episode number, for episodic names. */
  readonly episode: number | null
  /** Last episode number for combined files (`S01E01E02`). */
  readonly episodeEnd: number | null
  /** Confidence in the interpretation, `0..=1`. */
  readonly confidence: number
}

/** An episodic marker, once located in the name. */
interface Episodic {
  /** Where the marker starts, which is where the title ends. */
  readonly start: number
  readonly season: number
  readonly episode: number
  readonly end: number | null
}

/**
 * The file stem, with the same rule the Rust `Path::file_stem` applies.
 *
 * A leading dot is not an extension separator: `.hidden` is a name of its own in
 * Rust, and a port that used `name.split('.')[0]` would turn it into an empty string.
 */
export function fileStem(filename: string): string {
  const lastSeparator = Math.max(filename.lastIndexOf('/'), filename.lastIndexOf('\\'))
  const base = filename.slice(lastSeparator + 1)
  const dot = base.lastIndexOf('.')
  if (dot <= 0) {
    return base
  }
  return base.slice(0, dot)
}

/** The last component of a folder path, or an empty string. */
function lastFolderComponent(folder: string): string {
  const parts = folder.split(/[/\\]/).filter((part) => part.length > 0)
  return parts[parts.length - 1] ?? ''
}

/** The extension of a name, lowercased, without the dot, or `null`. */
export function extensionOf(name: string): string | null {
  const lastSeparator = Math.max(name.lastIndexOf('/'), name.lastIndexOf('\\'))
  const base = name.slice(lastSeparator + 1)
  const dot = base.lastIndexOf('.')
  if (dot <= 0 || dot === base.length - 1) {
    return null
  }
  return base.slice(dot + 1).toLowerCase()
}

/** A whole number from a capture group, or `null`. */
function numberOrNull(value: string | undefined): number | null {
  if (value === undefined) {
    return null
  }
  const parsed = Number.parseInt(value, 10)
  return Number.isSafeInteger(parsed) ? parsed : null
}

/**
 * Reduces a name to words: separators and brackets become spaces, runs collapse, and
 * a trailing dash or space is dropped.
 */
function clean(value: string): string {
  return value
    .replace(SEPARATORS, ' ')
    .replace(BRACKETS, ' ')
    .split(/\s+/)
    .filter((part) => part.length > 0)
    .join(' ')
    .replace(/[\s-]+$/, '')
}

/** The first episodic marker in a name, in either spelling. */
function findEpisodic(name: string): Episodic | null {
  const se = EPISODIC_SE.exec(name)
  const ax = se === null ? EPISODIC_AX.exec(name) : null
  const match = se ?? ax
  if (match === null || match.index === undefined) {
    return null
  }
  const season = numberOrNull(match[1])
  const episode = numberOrNull(match[2])
  if (season === null || episode === null) {
    return null
  }
  return { start: match.index, season, episode, end: numberOrNull(match[3]) }
}

/**
 * Interprets a filename, optionally using its containing folder for context.
 *
 * `folder` is a directory path or a plain folder name; only its last component is
 * consulted, and only when the filename itself yields no title.
 */
export function parseFilename(filename: string, folder?: string | null): ParsedName {
  const stem = fileStem(filename)
  let name = stem.replace(/_/g, '.')

  const episodic = findEpisodic(name)
  const yearMatch = YEAR.exec(name)
  const usableYear =
    yearMatch === null
      ? null
      : episodic !== null && yearMatch.index >= episodic.start
        ? null
        : yearMatch
  const yearValue = usableYear === null ? null : numberOrNull(usableYear[0])
  const year =
    yearValue !== null && yearValue >= 1800 && yearValue <= 2200 ? yearValue : null

  const cut =
    usableYear?.index ??
    episodic?.start ??
    (QUALITY.exec(name)?.index ?? null)
  if (cut !== null) {
    name = name.slice(0, cut)
  }

  let cleaned = clean(name)
  if (cleaned.length === 0 && folder !== undefined && folder !== null) {
    const label = clean(lastFolderComponent(folder))
    // A season folder is a position in a series, not a title; using it would name
    // twelve different files "Season 2".
    cleaned = SEASON_FOLDER.test(label) ? '' : label
  }

  const unknown = cleaned.length === 0 || NUMERIC_TITLE.test(cleaned)
  const season = episodic?.season ?? null
  const episode = episodic?.episode ?? null
  const episodeEnd =
    episodic?.end !== null &&
    episodic?.end !== undefined &&
    episode !== null &&
    episodic.end >= episode &&
    episodic.end - episode < 100
      ? episodic.end
      : episode

  const kind: MediaKind = episodic !== null ? 'series' : unknown ? 'other' : 'movie'
  const confidence = unknown ? 0.1 : episodic !== null ? 0.85 : year !== null ? 0.75 : 0.4

  return {
    title: unknown ? stem : cleaned,
    kind,
    year,
    season,
    episode,
    episodeEnd,
    confidence,
  }
}

/**
 * The key two files must share to be the same work.
 *
 * Ported from `Library::ingest_file` in the Rust core, including the threshold that
 * separates a confident match from an unlinked work. Below those thresholds a file
 * gets an identity of its own rather than being joined to something that merely looks
 * similar — the core's rule, and the reason a folder of loosely named files does not
 * collapse into one work.
 *
 * `unmatched` keys carry a unique suffix, so two files whose names cannot be read
 * never become one work. The suffix is passed in rather than generated here, because
 * this module is deterministic: a pure function that invented an id would return a
 * different key for the same input on every call.
 */
export function identityKeyOf(
  parsed: ParsedName,
  sourceId: string,
  unmatchedSuffix: string,
): string {
  const confidentSeries =
    parsed.kind === 'series' &&
    parsed.confidence >= 0.8 &&
    parsed.season !== null &&
    parsed.episode !== null
  if (confidentSeries) {
    return `series:${sourceId}:${parsed.title.toLowerCase()}:${parsed.year ?? ''}`
  }
  const confidentMovie =
    parsed.kind === 'movie' && parsed.year !== null && parsed.confidence >= 0.75
  if (confidentMovie) {
    return `movie:${parsed.title.toLowerCase()}:${parsed.year}`
  }
  return `unmatched:${unmatchedSuffix}`
}

/**
 * The folder hint for a path: the containing folder, with a trailing `Season N`
 * component removed.
 *
 * The Rust scanner does this before parsing, because `Show/Season 2/1080p.mkv` has no
 * title in the filename and the folder above the season folder is the only thing that
 * names the show.
 */
export function folderHintOf(relativePath: string): string {
  const separator = Math.max(relativePath.lastIndexOf('/'), relativePath.lastIndexOf('\\'))
  if (separator < 0) {
    return ''
  }
  const folder = relativePath.slice(0, separator)
  const last = lastFolderComponent(folder)
  if (!SEASON_FOLDER.test(last)) {
    return folder
  }
  const parentSeparator = Math.max(folder.lastIndexOf('/'), folder.lastIndexOf('\\'))
  return parentSeparator < 0 ? '' : folder.slice(0, parentSeparator)
}
