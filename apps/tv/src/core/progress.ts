/**
 * The playback-progress acceptance rule, ported from the Rust core.
 *
 * The core accepts progress only from the authoritative
 * `(session, generation, sequence)` triple, so a late native callback — from a
 * session that has already ended, or from a load that has already been replaced —
 * can never overwrite newer progress. On a TV this matters more than on the
 * desktop, not less: AVPlay reports position from its own thread, the user can press
 * Back mid-playback, and a request to write progress can be in flight while the
 * player is being torn down.
 *
 * Keeping the rule here, as a pure function, is what makes it testable without the
 * TV: the components only call it.
 */

import type { PlaybackSnapshot } from './contract.ts'

/** What the store should do with a snapshot it was handed. */
export type ProgressVerdict =
  /** The snapshot is newer and must be stored. */
  | { readonly accept: true; readonly snapshot: PlaybackSnapshot }
  /** The snapshot is stale or malformed; the stored value stands. */
  | { readonly accept: false; readonly reason: ProgressRejection; readonly snapshot: PlaybackSnapshot }

/** Why a snapshot was refused. Named so a log can say which rule fired. */
export type ProgressRejection =
  | 'malformed'
  | 'stale_session'
  | 'stale_generation'
  | 'stale_sequence'
  | 'same_sequence'

/**
 * The session that is allowed to write progress right now.
 *
 * The client creates the session id when it starts playback, so it — not the
 * callback — is what decides which session is live. Without this, a late callback
 * from an older session would look "newer" simply for being from a session we have
 * not seen before.
 */
export interface LiveSession {
  readonly sessionId: string
  /** The load of the file this session is currently on. */
  readonly generation: number
}

/** A snapshot whose numbers are usable as progress. */
function isWellFormed(snapshot: PlaybackSnapshot): boolean {
  return (
    Number.isSafeInteger(snapshot.generation) &&
    Number.isSafeInteger(snapshot.sequence) &&
    Number.isSafeInteger(snapshot.positionMs) &&
    Number.isSafeInteger(snapshot.durationMs) &&
    snapshot.positionMs >= 0 &&
    snapshot.durationMs >= 0
  )
}

/**
 * Decides whether `incoming` may replace `current`.
 *
 * `live` is the session the client believes is playing. A snapshot from any other
 * session is stale by definition, whatever its numbers say — that is the whole point
 * of the guard, because the sequence counter of an old session can be arbitrarily
 * high. Within the live session, a newer generation (a new load of the file) wins for
 * the same reason, and within one generation only a strictly higher sequence is
 * accepted, which is what stops a re-ordered callback from moving the playhead
 * backwards.
 */
export function acceptProgress(
  live: LiveSession,
  current: PlaybackSnapshot | null,
  incoming: PlaybackSnapshot,
): ProgressVerdict {
  if (!isWellFormed(incoming)) {
    return { accept: false, reason: 'malformed', snapshot: incoming }
  }
  if (incoming.sessionId !== live.sessionId) {
    return { accept: false, reason: 'stale_session', snapshot: current ?? incoming }
  }
  if (current === null) {
    return { accept: true, snapshot: incoming }
  }
  if (!isWellFormed(current) || current.sessionId !== incoming.sessionId) {
    // A stored value that cannot be trusted — or that came from another session —
    // must not block a good one.
    return { accept: true, snapshot: incoming }
  }
  if (incoming.generation < current.generation) {
    return { accept: false, reason: 'stale_generation', snapshot: current }
  }
  if (incoming.generation === current.generation) {
    if (incoming.sequence < current.sequence) {
      return { accept: false, reason: 'stale_sequence', snapshot: current }
    }
    if (incoming.sequence === current.sequence) {
      return { accept: false, reason: 'same_sequence', snapshot: current }
    }
  }
  return { accept: true, snapshot: incoming }
}

/**
 * True when a finished position should be recorded as *watched* rather than as a
 * resume point.
 *
 * `Ended` comes from the player, so this needs no guessed threshold: a file that
 * reported the end is finished. The ratio is only a fallback for a player that
 * stopped without reporting it.
 */
export const FINISHED_RATIO = 0.95

/** True when the position is at or past the finished threshold. */
export function isFinished(positionMs: number, durationMs: number, ended: boolean): boolean {
  if (ended) {
    return true
  }
  if (durationMs <= 0) {
    return false
  }
  return positionMs / durationMs >= FINISHED_RATIO
}

/**
 * How far into a file playback must be before the position is worth keeping.
 *
 * Below this a resume point is noise: the owner pressed play, watched the studio
 * logo, and stopped. `positionMs > 0` is the core's own rule for showing a resume
 * row, so anything above zero is *shown*; this is the narrower question of whether
 * it is worth *writing*, and the two are deliberately not the same number.
 */
export const WORTH_SAVING_MS = 15_000

/** True when a position is worth persisting for later. */
export function isWorthSaving(positionMs: number, durationMs: number): boolean {
  if (positionMs <= 0) {
    return false
  }
  // For a long file, fifteen seconds. For a short one, fifteen seconds may be most
  // of it, so one percent stands in — whichever is smaller.
  const floor =
    durationMs > 0 ? Math.min(WORTH_SAVING_MS, Math.floor(durationMs * 0.01)) : WORTH_SAVING_MS
  return positionMs >= floor
}
