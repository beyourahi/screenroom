/**
 * The storage port, and the service that owns durable state.
 *
 * Everything with a decision in it lives here, so the platform file underneath is
 * just bytes in and bytes out. That is what makes the durability rules testable: the
 * tests drive this service with an in-memory port and can make writes fail on
 * purpose, which is the only way to check that a power cut cannot lose a library.
 *
 * The port is deliberately tiny and deliberately total:
 *
 * * `read()` returns the stored text, or `null` when there is nothing stored. A read
 *   failure also returns `null` — the service then reports a reset, rather than
 *   refusing to start.
 * * `write()` returns whether the bytes were accepted. Tizen's `localStorage` throws
 *   when the quota is exceeded, and a quota failure must be *reported*, not assumed
 *   to have worked.
 */

import type { LibraryItem, PlaybackSnapshot, Profile, ProfileId, Source } from './contract.ts'
import type { ScanReport } from './scan.ts'
import type { DeviceSettings } from './settings.ts'
import { acceptProgress, isFinished, isWorthSaving, type LiveSession } from './progress.ts'
import {
  decodeSnapshot,
  emptySnapshot,
  encodeSnapshot,
  withHiddenWork,
  withItemProgress,
  withScanReport,
  withSettings,
  type StoreIssue,
  type StoreSnapshot,
} from './store.ts'

/** Where the document is kept. */
export interface StoragePort {
  /** The stored document, or `null` when there is none or it could not be read. */
  read(): string | null
  /** Stores the document; returns false when the platform refused it. */
  write(document: string): boolean
  /** Removes the document. Used by an explicit "reset this device" action. */
  clear(): void
  /** A short description of where state lives, for the Settings screen. */
  describe(): string
}

/** An in-memory port, for tests and for a browser preview. */
export function createMemoryStorage(initial: string | null = null): StoragePort & {
  readonly document: () => string | null
} {
  let stored = initial
  return {
    read: () => stored,
    write: (document: string) => {
      stored = document
      return true
    },
    clear: () => {
      stored = null
    },
    describe: () => 'this tab only',
    document: () => stored,
  }
}

/** What the service reports about the last load. */
export interface LoadReport {
  /** True when stored state existed but could not be used. */
  readonly lostStoredState: boolean
  readonly issues: readonly StoreIssue[]
  /** Where the state lives, for the Settings screen. */
  readonly location: string
}

/** The last write that did not reach the device, if any. */
export interface WriteFailure {
  readonly reason: string
}

/**
 * The library service.
 *
 * One instance owns the snapshot for the whole app, the way the Rust core owns one
 * connection for the whole desktop app.
 */
export class LibraryService {
  private snapshot: StoreSnapshot
  private readonly port: StoragePort
  private readonly now: () => number
  private lastLoad: LoadReport
  private writeFailed: WriteFailure | null = null
  private lastWritten: string | null = null

  constructor(port: StoragePort, now: () => number = () => Date.now()) {
    this.port = port
    this.now = now
    const result = decodeSnapshot(port.read())
    this.snapshot = result.snapshot
    this.lastLoad = {
      lostStoredState: result.reset,
      issues: result.issues,
      location: port.describe(),
    }
    this.lastWritten = result.reset ? null : encodeSnapshot(this.snapshot)
  }

  /** What the last load found. Shown in Settings; never swallowed. */
  loadReport(): LoadReport {
    return this.lastLoad
  }

  /** The write that failed, until a later write succeeds. */
  writeFailure(): WriteFailure | null {
    return this.writeFailed
  }

  /** The whole snapshot. Callers must not mutate it. */
  state(): StoreSnapshot {
    return this.snapshot
  }

  /** Every catalogued item. */
  items(): readonly LibraryItem[] {
    return this.snapshot.items
  }

  /** The viewer profiles on this device. */
  profiles(): readonly Profile[] {
    return this.snapshot.profiles
  }

  /** The profile in use, or `null` before one has been chosen. */
  activeProfileId(): ProfileId | null {
    return this.snapshot.activeProfileId
  }

  /** A source by id, or `null`. */
  source(id: string) {
    return this.snapshot.sources.find((source) => source.id === id) ?? null
  }

  /** True when a work's resume point is hidden. */
  isHidden(workId: string): boolean {
    return this.snapshot.hiddenWorkIds.includes(workId)
  }

  /** Chooses the viewer profile. */
  setActiveProfile(profileId: ProfileId | null): boolean {
    if (profileId !== null && !this.snapshot.profiles.some((p) => p.id === profileId)) {
      return false
    }
    this.snapshot = { ...this.snapshot, activeProfileId: profileId }
    return this.persist()
  }

  /** Adds a profile and returns its id. */
  addProfile(name: string): ProfileId | null {
    const trimmed = name.trim()
    if (trimmed.length === 0) {
      return null
    }
    const id = nextId('profile', this.snapshot.profiles.length, this.now())
    const profile: Profile = { id, name: trimmed }
    this.snapshot = {
      ...this.snapshot,
      profiles: [...this.snapshot.profiles, profile],
      // The first profile becomes the active one; a TV with one profile must not
      // start on "no profile".
      activeProfileId: this.snapshot.activeProfileId ?? id,
    }
    this.persist()
    return id
  }

  /**
   * Records playback progress.
   *
   * The snapshot must come from the live session, and only a higher sequence within
   * it is accepted — see `progress.ts`. Returns whether the stored state changed, so
   * the caller can decide not to write: `localStorage` on a 2021 TV is small and
   * synchronous, and a write per player tick would be felt in the frame budget.
   */
  recordProgress(
    live: LiveSession,
    incoming: PlaybackSnapshot,
    workId: string | null,
  ): boolean {
    const itemId = this.itemIdForSnapshot(incoming)
    if (itemId === null) {
      return false
    }
    const verdict = acceptProgress(live, this.acceptedFor(itemId), incoming)
    if (!verdict.accept) {
      return false
    }
    this.accepted.set(itemId, verdict.snapshot)

    const finished = isFinished(incoming.positionMs, incoming.durationMs, incoming.ended)
    const position = finished ? 0 : incoming.positionMs
    if (!finished && !isWorthSaving(position, incoming.durationMs)) {
      // Too early to be worth keeping. The stored position stands; the in-memory
      // accepted snapshot still advanced, so the next tick is compared against it.
      return false
    }
    this.snapshot = withItemProgress(
      this.snapshot,
      itemId,
      position,
      incoming.durationMs,
      finished,
      this.now(),
    )
    if (finished && workId !== null) {
      // Finishing something is the one moment a hidden resume point stops being
      // meaningful, so it is forgotten rather than left to reappear later.
      this.snapshot = withHiddenWork(this.snapshot, workId, false)
    }
    return this.persist()
  }

  /** Hides or unhides a work's resume point, as the Detail screen offers. */
  setResumeHidden(workId: string, hidden: boolean): boolean {
    this.snapshot = withHiddenWork(this.snapshot, workId, hidden)
    return this.persist()
  }

  /** Marks a work watched or unwatched, clearing or keeping its position. */
  setWatched(itemId: string, watched: boolean): boolean {
    const item = this.snapshot.items.find((candidate) => candidate.id === itemId)
    if (item === undefined) {
      return false
    }
    this.snapshot = withItemProgress(
      this.snapshot,
      itemId,
      watched ? 0 : item.positionMs,
      item.durationMs,
      watched,
      this.now(),
    )
    return this.persist()
  }

  /** Replaces the catalog read from a scan, keeping progress for ids that survive. */
  replaceCatalog(
    items: readonly LibraryItem[],
    sources: StoreSnapshot['sources'],
  ): boolean {
    const previous = new Map(this.snapshot.items.map((item) => [item.id, item]))
    const merged = items.map((item) => {
      const before = previous.get(item.id)
      if (before === undefined) {
        return item
      }
      return {
        ...item,
        // Progress is the owner's, not the scanner's: a rescan must never reset a
        // position it did not measure.
        positionMs: before.positionMs,
        durationMs: item.durationMs > 0 ? item.durationMs : before.durationMs,
        watched: before.watched,
        updatedMs: before.updatedMs,
      }
    })
    this.snapshot = { ...this.snapshot, items: merged, sources }
    return this.persist()
  }

  /**
   * Replaces the catalogue, the sources and the scan report in one write.
   *
   * One write rather than three, because `localStorage` on a 2021 TV is synchronous and
   * small: a scan that wrote the items, then the sources, then the report would be
   * three chances to be interrupted halfway through a library update.
   *
   * Item progress is still carried forward per id, exactly as `replaceCatalog` does.
   */
  applyScan(input: {
    readonly items: readonly LibraryItem[]
    readonly sources: readonly Source[]
    readonly report: ScanReport
  }): boolean {
    const previous = new Map(this.snapshot.items.map((item) => [item.id, item]))
    const merged = input.items.map((item) => {
      const before = previous.get(item.id)
      if (before === undefined) {
        return item
      }
      return {
        ...item,
        positionMs: before.positionMs,
        durationMs: item.durationMs > 0 ? item.durationMs : before.durationMs,
        watched: before.watched,
        updatedMs: before.updatedMs,
      }
    })
    this.snapshot = {
      ...this.snapshot,
      items: merged,
      sources: input.sources,
      lastScan: input.report,
    }
    return this.persist()
  }

  /** This device's playback preferences. */
  settings(): DeviceSettings {
    return this.snapshot.settings
  }

  /**
   * Stores new playback preferences.
   *
   * `false` means "nothing was written", which is also what an unchanged preference
   * reports: the writer compares the encoded document, so setting the same language
   * twice costs no flash write.
   */
  saveSettings(settings: DeviceSettings): boolean {
    this.snapshot = withSettings(this.snapshot, settings)
    return this.persist()
  }

  /** What the last scan did, as it was stored. */
  lastScan(): ScanReport {
    return this.snapshot.lastScan
  }

  /** Records a finished scan where no catalogue changed. */
  recordScan(report: ScanReport): boolean {
    this.snapshot = withScanReport(this.snapshot, report)
    return this.persist()
  }

  /** Writes the current state, reporting a refusal rather than assuming success. */
  private persist(): boolean {
    const document = encodeSnapshot(this.snapshot)
    if (document === this.lastWritten) {
      return false
    }
    const accepted = this.port.write(document)
    if (accepted) {
      this.lastWritten = document
      this.writeFailed = null
      return true
    }
    this.writeFailed = {
      reason: 'The device refused the write. Storage may be full or the browser blocked it.',
    }
    return false
  }

  /** The accepted snapshot per item, which is what the guard compares against. */
  private readonly accepted = new Map<string, PlaybackSnapshot>()

  private acceptedFor(itemId: string): PlaybackSnapshot | null {
    return this.accepted.get(itemId) ?? null
  }

  /**
   * The item a snapshot belongs to.
   *
   * The player is given the session id when it starts, so the item is looked up
   * through the session that was handed out rather than through the snapshot, which
   * carries no item id — a snapshot from a session that is not live has nowhere to
   * land, which is the point.
   */
  private itemIdForSnapshot(snapshot: PlaybackSnapshot): string | null {
    return this.sessionItems.get(snapshot.sessionId) ?? null
  }

  private readonly sessionItems = new Map<string, string>()

  /** Registers a session as playing one item, which is what makes its progress valid. */
  beginSession(sessionId: string, itemId: string): void {
    this.sessionItems.set(sessionId, itemId)
  }

  /** Ends a session, so a late callback from it can no longer write anything. */
  endSession(sessionId: string): void {
    this.sessionItems.delete(sessionId)
  }
}

/** A stable, sortable id. Generated here, never derived from a path. */
export function nextId(prefix: string, sequence: number, timestampMs: number): string {
  const time = Math.max(0, Math.trunc(timestampMs)).toString(36)
  return `${prefix}-${time}-${sequence.toString(36)}`
}

/** An empty snapshot, re-exported so callers need only this module. */
export { emptySnapshot }
export type { StoreSnapshot }
