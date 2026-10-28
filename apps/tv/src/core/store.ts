/**
 * The device-local store, as a versioned document.
 *
 * The TV has no Rust core, so its state has to live somewhere the platform offers.
 * Tizen web apps get `localStorage` and a writable application data area, not SQLite
 * — which is allowed by ADR-003 ("SQLite where the platform supports it; a documented
 * adapter with the same entities and invariants where it does not") but only if the
 * adapter keeps those invariants. So this file is where the invariants live, and the
 * platform's I/O is a thin layer on top of it.
 *
 * The invariants it keeps:
 *
 * * **Nothing is silently reset.** A row that cannot be read is dropped and
 *   *counted*, so the screen can say so. One corrupt entry never costs the whole
 *   library, because the owner's history is not recoverable from anywhere else.
 * * **A newer format is never guessed at.** A document written by a future version is
 *   refused with a truthful reason rather than half-parsed.
 * * **Input is bounded.** The parser has a length ceiling and a row ceiling, because
 *   this input comes from a file on disk that anything could have written.
 * * **Identity is the app's own id, never a path.** The document stores paths only as
 *   listener data (the granted roots), never as identity.
 */

import type {
  LibraryItem,
  MediaKind,
  Profile,
  ProfileId,
  Source,
  SourceState,
} from './contract.ts'
import { decodeScanReport, NEVER_SCANNED, type ScanReport } from './scan.ts'
import { decodeSettings, DEFAULT_SETTINGS, type DeviceSettings } from './settings.ts'

/**
 * Version of the document this build writes.
 *
 * 2 added the playback preferences and the last scan report. A version 1 document is
 * still read: the two new fields are absent, so their decoders return the defaults —
 * which is the whole point of keeping the version below a refusal threshold. Only a
 * document written by a *newer* build is refused, because nothing here can know what
 * such a build meant by a field that looks familiar.
 */
export const STORE_FORMAT_VERSION = 2

/** Largest document this parser will look at, in characters. */
export const MAX_STORE_CHARS = 8 * 1024 * 1024

/** Largest number of rows per collection this parser will accept. */
export const MAX_ROWS = 200_000

/** The whole device-local state. */
export interface StoreSnapshot {
  readonly formatVersion: number
  readonly profiles: readonly Profile[]
  readonly sources: readonly Source[]
  readonly items: readonly LibraryItem[]
  /** The profile the TV was last used as. */
  readonly activeProfileId: ProfileId | null
  /** Per-work "never show a resume point" choices, by work id. */
  readonly hiddenWorkIds: readonly string[]
  /** This device's playback preferences, learned from the player's own menu. */
  readonly settings: DeviceSettings
  /** What the last scan did, so a restart can still say what it found. */
  readonly lastScan: ScanReport
}

/** An empty store, as a brand-new install has. */
export function emptySnapshot(): StoreSnapshot {
  return {
    formatVersion: STORE_FORMAT_VERSION,
    profiles: [],
    sources: [],
    items: [],
    activeProfileId: null,
    hiddenWorkIds: [],
    settings: DEFAULT_SETTINGS,
    lastScan: NEVER_SCANNED,
  }
}

/** A row the reader refused, and why. Reported, never swallowed. */
export interface StoreIssue {
  readonly collection: 'profiles' | 'sources' | 'items' | 'document'
  readonly reason: string
  /** The index in the collection, or -1 for a document-level issue. */
  readonly index: number
}

/** What reading a document produced. */
export interface StoreReadResult {
  readonly snapshot: StoreSnapshot
  /** True when the stored bytes were not usable and an empty store stands in. */
  readonly reset: boolean
  readonly issues: readonly StoreIssue[]
}

const SOURCE_STATES: readonly SourceState[] = [
  'available',
  'disconnected',
  'permissionLost',
  'missing',
  'removed',
]

const KINDS: readonly MediaKind[] = ['movie', 'series', 'other']

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** A whole number, or `null` when the value is not one. */
function wholeOrNull(value: unknown): number | null {
  return typeof value === 'number' && Number.isSafeInteger(value) ? value : null
}

/** A whole number, or the fallback. */
function whole(value: unknown, fallback = 0): number {
  return wholeOrNull(value) ?? fallback
}

function text(value: unknown): string | null {
  return typeof value === 'string' ? value : null
}

function boolean(value: unknown): boolean {
  return value === true
}

/** Reads one profile row, or `null` when it cannot be used. */
function readProfile(row: unknown): Profile | null {
  if (!isRecord(row)) {
    return null
  }
  const id = text(row['id'])
  const name = text(row['name'])
  if (id === null || id.length === 0 || name === null) {
    return null
  }
  return { id, name }
}

/** Reads one source row, or `null` when it cannot be used. */
function readSource(row: unknown): Source | null {
  if (!isRecord(row)) {
    return null
  }
  const id = text(row['id'])
  const label = text(row['label'])
  const rootPath = text(row['rootPath'])
  const state = text(row['state'])
  if (id === null || id.length === 0 || label === null || rootPath === null || state === null) {
    return null
  }
  if (!SOURCE_STATES.includes(state as SourceState)) {
    return null
  }
  return { id, label, rootPath, state: state as SourceState, lastScanMs: whole(row['lastScanMs']) }
}

/** Reads one item row, or `null` when it cannot be used. */
function readItem(row: unknown): LibraryItem | null {
  if (!isRecord(row)) {
    return null
  }
  const id = text(row['id'])
  const workId = text(row['workId'])
  const title = text(row['title'])
  const kind = text(row['kind'])
  const relativePath = text(row['relativePath'])
  const sourceId = text(row['sourceId'])
  if (
    id === null ||
    id.length === 0 ||
    workId === null ||
    workId.length === 0 ||
    title === null ||
    relativePath === null ||
    sourceId === null ||
    kind === null ||
    !KINDS.includes(kind as MediaKind)
  ) {
    return null
  }
  return {
    id,
    workId,
    title,
    kind: kind as MediaKind,
    season: wholeOrNull(row['season']),
    episode: wholeOrNull(row['episode']),
    year: wholeOrNull(row['year']),
    relativePath,
    sourceId,
    available: boolean(row['available']),
    favorite: boolean(row['favorite']),
    watched: boolean(row['watched']),
    hidden: boolean(row['hidden']),
    positionMs: Math.max(0, whole(row['positionMs'])),
    durationMs: Math.max(0, whole(row['durationMs'])),
    updatedMs: Math.max(0, whole(row['updatedMs'])),
  }
}

/** Reads a collection, reporting every row that was refused. */
function readCollection<T>(
  value: unknown,
  collection: StoreIssue['collection'],
  read: (row: unknown) => T | null,
  issues: StoreIssue[],
): T[] {
  if (!Array.isArray(value)) {
    if (value !== undefined) {
      issues.push({ collection, reason: 'not a list', index: -1 })
    }
    return []
  }
  if (value.length > MAX_ROWS) {
    issues.push({ collection, reason: `more than ${MAX_ROWS} rows`, index: -1 })
    return []
  }
  const rows: T[] = []
  value.forEach((row, index) => {
    const parsed = read(row)
    if (parsed === null) {
      issues.push({ collection, reason: 'unreadable row', index })
      return
    }
    rows.push(parsed)
  })
  return rows
}

/**
 * Reads a stored document.
 *
 * Never throws: a store that cannot be read returns an empty snapshot with
 * `reset: true` and the reasons, because an app that refuses to start because its
 * own cache is damaged is worse than one that starts and says what it lost.
 */
export function decodeSnapshot(raw: string | null): StoreReadResult {
  if (raw === null || raw.length === 0) {
    return { snapshot: emptySnapshot(), reset: false, issues: [] }
  }
  if (raw.length > MAX_STORE_CHARS) {
    return {
      snapshot: emptySnapshot(),
      reset: true,
      issues: [
        { collection: 'document', reason: `larger than ${MAX_STORE_CHARS} characters`, index: -1 },
      ],
    }
  }

  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return {
      snapshot: emptySnapshot(),
      reset: true,
      issues: [{ collection: 'document', reason: 'not valid JSON', index: -1 }],
    }
  }

  if (!isRecord(parsed)) {
    return {
      snapshot: emptySnapshot(),
      reset: true,
      issues: [{ collection: 'document', reason: 'not an object', index: -1 }],
    }
  }

  const version = whole(parsed['formatVersion'], 0)
  if (version > STORE_FORMAT_VERSION) {
    // Refused rather than guessed at: a newer writer may mean something different by
    // a field that looks identical, and a half-parsed library is worse than an
    // honest refusal.
    return {
      snapshot: emptySnapshot(),
      reset: true,
      issues: [
        {
          collection: 'document',
          reason: `written by a newer version (${version})`,
          index: -1,
        },
      ],
    }
  }

  const issues: StoreIssue[] = []
  const profiles = readCollection(parsed['profiles'], 'profiles', readProfile, issues)
  const sources = readCollection(parsed['sources'], 'sources', readSource, issues)
  const items = readCollection(parsed['items'], 'items', readItem, issues)

  const activeProfileId = text(parsed['activeProfileId'])
  const hiddenWorkIds = Array.isArray(parsed['hiddenWorkIds'])
    ? parsed['hiddenWorkIds'].filter((value): value is string => typeof value === 'string')
    : []

  return {
    snapshot: {
      formatVersion: STORE_FORMAT_VERSION,
      profiles,
      sources,
      items,
      // A remembered profile that no longer exists is not an error, but it must not
      // be handed back as current either.
      activeProfileId:
        activeProfileId !== null && profiles.some((profile) => profile.id === activeProfileId)
          ? activeProfileId
          : null,
      hiddenWorkIds,
      settings: decodeSettings(parsed['settings']),
      lastScan: decodeScanReport(parsed['lastScan']),
    },
    reset: false,
    issues,
  }
}

/**
 * Writes a document.
 *
 * Field order is fixed and the output is compact, so a diff of two saves is readable
 * and a save of unchanged state produces identical bytes — which is what lets the
 * writer skip a write it does not need.
 */
export function encodeSnapshot(snapshot: StoreSnapshot): string {
  return JSON.stringify({
    formatVersion: STORE_FORMAT_VERSION,
    profiles: snapshot.profiles,
    sources: snapshot.sources,
    items: snapshot.items,
    activeProfileId: snapshot.activeProfileId,
    hiddenWorkIds: snapshot.hiddenWorkIds,
    settings: snapshot.settings,
    lastScan: snapshot.lastScan,
  })
}

/**
 * Replaces one item's playback state, returning a new snapshot.
 *
 * Immutable on purpose: the caller decides when to write, so a burst of player
 * callbacks produces one document rather than one per callback.
 */
export function withItemProgress(
  snapshot: StoreSnapshot,
  itemId: string,
  positionMs: number,
  durationMs: number,
  watched: boolean,
  updatedMs: number,
): StoreSnapshot {
  let changed = false
  const items = snapshot.items.map((item) => {
    if (item.id !== itemId) {
      return item
    }
    changed = true
    return {
      ...item,
      positionMs: Math.max(0, Math.trunc(positionMs)),
      durationMs: Math.max(0, Math.trunc(durationMs)),
      watched,
      updatedMs,
    }
  })
  return changed ? { ...snapshot, items } : snapshot
}

/** Replaces the device's playback preferences. */
export function withSettings(snapshot: StoreSnapshot, settings: DeviceSettings): StoreSnapshot {
  return { ...snapshot, settings }
}

/** Records what a scan did. */
export function withScanReport(snapshot: StoreSnapshot, report: ScanReport): StoreSnapshot {
  return { ...snapshot, lastScan: report }
}

/** Sets a work's resume point to hidden, or stops hiding it. */
export function withHiddenWork(
  snapshot: StoreSnapshot,
  workId: string,
  hidden: boolean,
): StoreSnapshot {
  const has = snapshot.hiddenWorkIds.includes(workId)
  if (hidden === has) {
    return snapshot
  }
  return {
    ...snapshot,
    hiddenWorkIds: hidden
      ? [...snapshot.hiddenWorkIds, workId]
      : snapshot.hiddenWorkIds.filter((id) => id !== workId),
  }
}
