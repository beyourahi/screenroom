/**
 * The Screenroom core contract, as the TV client sees it.
 *
 * This is a *port of the contract*, not of the implementation. The TV client does
 * not run the Rust core (`crates/screenroom-core`): it speaks the same entities and
 * the same invariants over its own storage adapter. Every type here has a
 * counterpart in `crates/screenroom-core/src/domain.rs`, and a field that exists
 * there and not here is a field this client cannot show — which is honest, whereas
 * inventing a field would be a lie the desktop could never confirm.
 *
 * Two rules travel with the contract and are enforced in code, not by convention:
 *
 * 1. **Identity is an application-generated id, never a path, filename or provider
 *    id.** `WorkId`, `EditionId`, `ItemId` and `SourceId` are opaque strings here for
 *    exactly that reason; nothing may parse them.
 * 2. **Availability never deletes.** A source that is unreadable, disconnected or
 *    removed hides its rows and keeps every work, progress position and list entry.
 */

/** Opaque application-generated identifier for a work (a film, or a series). */
export type WorkId = string
/** Opaque identifier for one edition of a work (a release, a cut, a version). */
export type EditionId = string
/** Opaque identifier for one catalogued file. */
export type ItemId = string
/** Opaque identifier for an authorized media root. */
export type SourceId = string
/** Opaque identifier for a viewer profile. */
export type ProfileId = string

/**
 * Stable machine-readable failures, mirroring `crates/screenroom-core/src/error.rs`.
 *
 * UI copy is chosen from these codes. The message that travels beside a code is a
 * human-readable fallback and is not a stable contract.
 */
export type CoreErrorCode =
  | 'duplicate_source'
  | 'invalid_root'
  | 'source_missing'
  | 'source_disconnected'
  | 'source_removed'
  | 'permission_lost'
  | 'unsafe_path'
  | 'still_downloading'
  | 'cancelled'
  | 'database_error'
  | 'io_error'
  | 'invalid_stored_json'

/** A core failure with a stable machine-readable code. */
export interface CoreFailure {
  readonly code: CoreErrorCode
  readonly message: string
}

/** True when a failure is expected while a device or source is offline. */
export function isAvailabilityFailure(code: CoreErrorCode): boolean {
  return (
    code === 'source_missing' ||
    code === 'source_disconnected' ||
    code === 'permission_lost' ||
    code === 'io_error'
  )
}

/**
 * Availability of an authorized media root.
 *
 * `PermissionLost` is deliberately distinct from `Disconnected` and `Missing`,
 * because each has a different recovery action: grant access again, attach the
 * drive again, or relink the folder.
 */
export type SourceState =
  | 'available'
  | 'disconnected'
  | 'permissionLost'
  | 'missing'
  | 'removed'

/** An authorized media root. */
export interface Source {
  readonly id: SourceId
  readonly label: string
  /** The granted root. Stored as an opaque string; never parsed for identity. */
  readonly rootPath: string
  readonly state: SourceState
  readonly lastScanMs: number
}

/** What a catalogued file is. */
export type MediaKind = 'movie' | 'series' | 'other'

/** A viewer profile. Per-device state is independent by design; there is no sync. */
export interface Profile {
  readonly id: ProfileId
  readonly name: string
}

/**
 * One catalogued item, joined with the playback state of one profile.
 *
 * This mirrors the shape `Library::items` returns on the desktop, including the
 * per-profile fields, so a screen written against this contract could be driven by
 * the Rust core unchanged.
 */
export interface LibraryItem {
  readonly id: ItemId
  readonly workId: WorkId
  readonly title: string
  readonly kind: MediaKind
  readonly season: number | null
  readonly episode: number | null
  readonly year: number | null
  /** Path relative to its source root. Validated before use; never absolute. */
  readonly relativePath: string
  readonly sourceId: SourceId
  /** False when the source is not available. Rows stay; only playback is blocked. */
  readonly available: boolean
  readonly favorite: boolean
  readonly watched: boolean
  readonly hidden: boolean
  /** Resume position in whole milliseconds, as the core stores it. */
  readonly positionMs: number
  /** Duration in whole milliseconds, or 0 when it has never been measured. */
  readonly durationMs: number
  /** When this item's playback state last changed, in whole milliseconds. */
  readonly updatedMs: number
}

/**
 * The authoritative playback state of one player session.
 *
 * `sessionId`, `generation` and `sequence` are the core's three-part guard: a late
 * native callback from a previous session or a previous load can never overwrite
 * newer progress. See `progress.ts`, which is where that rule is enforced.
 */
export interface PlaybackSnapshot {
  readonly sessionId: string
  readonly generation: number
  readonly sequence: number
  readonly positionMs: number
  readonly durationMs: number
  /** True when the player reached the end of the file. */
  readonly ended: boolean
  /** True when playback is paused. */
  readonly paused: boolean
}

/** A resume entry for the Home screen, ordered most recent first. */
export interface ResumeEntry {
  readonly item: LibraryItem
  readonly positionMs: number
  readonly durationMs: number
}
