/**
 * The playback runtime: one decoder, one session, one checkpoint clock.
 *
 * This lives outside the screen on purpose. The screen's job is to draw and to turn key
 * presses into calls; the job of remembering which session is live, how many
 * checkpoints have gone out and when the last one was written is *not* a drawing
 * concern, and putting it in component state would make every position report a
 * reactive write — on a TV, that is a frame budget spent on a number nobody is looking
 * at until the overlay is up.
 *
 * Everything mutable here is per-playback and cleared by `end()`, so the app can never
 * hold a decoder it has lost track of.
 */

import type { PlaybackSnapshot } from '../core/contract.ts'
import { library } from './runtime.ts'
import { startPlayback, type PlaybackEvents, type PlaybackSession } from '../platform/playback.ts'

/** How often a position is persisted while playing, in milliseconds. */
export const CHECKPOINT_MS = 5_000

/** The three-part identity the store's guard accepts progress from. */
const GENERATION = 1

let session: PlaybackSession | null = null
let sessionId = ''
let itemId = ''
let workId = ''
let sequence = 0
let lastCheckpointMs = 0
let positionMs = 0
let durationMs = 0

/** What the screen needs in order to draw. */
export interface PlaybackView {
  readonly positionMs: number
  readonly durationMs: number
  readonly paused: boolean
  readonly running: boolean
}

/** Opens a file. Returns false when a playback was already in progress and was ended. */
export function begin(
  item: { readonly id: string; readonly workId: string },
  uri: string,
  startMs: number,
  events: PlaybackEvents,
): boolean {
  const restarted = session !== null
  if (restarted) {
    end()
  }
  sessionId = `play-${item.id}-${String(Date.now())}`
  itemId = item.id
  workId = item.workId
  sequence = 0
  positionMs = startMs
  durationMs = 0
  lastCheckpointMs = startMs
  // Registering the session is what makes its progress valid: a snapshot carrying any
  // other session id has nowhere to land.
  library().beginSession(sessionId, item.id)
  session = startPlayback(uri, startMs, {
    onReady: events.onReady,
    onPosition: (milliseconds: number) => {
      positionMs = milliseconds
      events.onPosition(milliseconds)
    },
    onDuration: (milliseconds: number) => {
      durationMs = milliseconds
      events.onDuration(milliseconds)
    },
    onEnded: events.onEnded,
    onError: events.onError,
    onBuffering: events.onBuffering,
  })
  return !restarted
}

/** True when a decoder is open. */
export function running(): boolean {
  return session !== null
}

/** What the screen should draw. */
export function view(): PlaybackView {
  return {
    positionMs,
    durationMs,
    paused: session?.isPaused() ?? false,
    running: session !== null,
  }
}

/** Pauses, if playing, and checkpoints immediately. */
export function pause(): void {
  if (session === null) {
    return
  }
  if (!session.isPaused()) {
    session.pause()
    checkpoint(false)
  }
}

/** Resumes, if paused. */
export function resume(): void {
  session?.play()
}

/** Seeks by an absolute amount. */
export function seekTo(milliseconds: number): void {
  session?.seekTo(milliseconds)
}

/** Seeks relative to the current position, by the platform's own jump. */
export function jump(deltaMilliseconds: number): void {
  session?.jump(deltaMilliseconds)
}

/** The tracks the decoder offers, for a future track menu. */
export function tracks() {
  return session?.tracks() ?? []
}

/** Writes a checkpoint when enough time has passed since the last one. */
export function checkpointIfDue(): void {
  if (Math.abs(positionMs - lastCheckpointMs) < CHECKPOINT_MS) {
    return
  }
  checkpoint(false)
}

/**
 * Persists the current position.
 *
 * The snapshot carries the session the app handed out, the load generation and a
 * strictly increasing sequence, which is exactly what `core/progress.ts` needs in order
 * to refuse a late callback from a session that has already ended or a load that has
 * already been replaced.
 */
export function checkpoint(ended: boolean): void {
  if (sessionId === '' || itemId === '') {
    return
  }
  sequence += 1
  const snapshot: PlaybackSnapshot = {
    sessionId,
    generation: GENERATION,
    sequence,
    positionMs,
    durationMs,
    ended,
    paused: session?.isPaused() ?? false,
  }
  library().recordProgress({ sessionId, generation: GENERATION }, snapshot, workId)
  lastCheckpointMs = positionMs
}

/**
 * Saves the final position and closes the decoder.
 *
 * Order matters: the session is deregistered *before* the decoder is torn down, so a
 * callback that fires during teardown is refused rather than written on top of the
 * final position.
 */
export function end(): void {
  if (session === null) {
    return
  }
  checkpoint(false)
  library().endSession(sessionId)
  session.dispose()
  session = null
  sessionId = ''
  itemId = ''
  workId = ''
  sequence = 0
  lastCheckpointMs = 0
  positionMs = 0
  durationMs = 0
}
