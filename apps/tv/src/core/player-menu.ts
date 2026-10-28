/**
 * The player's controls, as a menu the D-pad can reach.
 *
 * The player used to be the one screen whose controls lived on *keys*: play/pause was
 * the play key, a minute was rewind and forward, and the track menu was Up. A Samsung
 * remote has all of those, but a viewer who does not know that — or who is holding a
 * simpler remote — had a picture and no controls at all.
 *
 * So the controls are rows in the same flat list the rest of the app uses. Up or Down
 * opens the menu, Up and Down move inside it, OK runs the highlighted row. The media
 * keys still work as shortcuts; nothing *needs* them.
 *
 * Order is deliberate: the three controls a viewer reaches for most first, then the next
 * episode when one exists, then the tracks the file reported, then the way out. A menu
 * whose first row is a language selector is a menu built for the file, not for the
 * person.
 */

import { buildTrackMenu, type TrackChoice, type TrackState } from './tracks.ts'

/** How far the menu's seek rows move, in milliseconds. */
export const MENU_SEEK_MS = 10_000

/** One row of the player menu. */
export type PlayerMenuKind =
  | 'playpause'
  | 'seekBack'
  | 'seekForward'
  | 'next'
  | 'audio'
  | 'text'
  | 'off'
  | 'exit'

/** What choosing one row does. */
export interface PlayerMenuRow {
  readonly key: string
  readonly kind: PlayerMenuKind
  /** The track this row selects, for the three track kinds. */
  readonly track: TrackChoice | null
  readonly label: string
  readonly detail: string
  /** True for the row describing what is already selected. */
  readonly selected: boolean
}

/** Everything the menu needs to know about the file on screen. */
export interface PlayerMenuInput {
  readonly tracks: readonly TrackChoice[]
  readonly state: TrackState
  readonly paused: boolean
  /**
   * False when no decoder session is running — a file that would not open, or one that
   * has already been closed.
   *
   * The pause and seek rows are left out in that case. A Pause row over a black screen
   * is a control that looks operable and does nothing, and the one honest control for a
   * file that did not play is the way out.
   */
  readonly running?: boolean
  /** True once the file has ended, which is when a next episode may be offered. */
  readonly finished: boolean
  /** The verified next episode's label, or empty when there is none. */
  readonly nextLabel: string
}

/** The menu, controls first. */
export function buildPlayerMenu(input: PlayerMenuInput): PlayerMenuRow[] {
  const rows: PlayerMenuRow[] = []
  if (input.running !== false) {
    rows.push(
      {
        key: 'control:playpause',
        kind: 'playpause',
        track: null,
        label: input.paused ? 'Play' : 'Pause',
        detail: input.paused ? 'Continues from where the picture stopped.' : 'Holds the picture where it is.',
        selected: false,
      },
      {
        key: 'control:seekBack',
        kind: 'seekBack',
        track: null,
        label: `Back ${MENU_SEEK_MS / 1000} seconds`,
        detail: 'Steps back through the file.',
        selected: false,
      },
      {
        key: 'control:seekForward',
        kind: 'seekForward',
        track: null,
        label: `Forward ${MENU_SEEK_MS / 1000} seconds`,
        detail: 'Steps forward through the file.',
        selected: false,
      },
    )
  }

  if (input.finished && input.nextLabel !== '') {
    rows.push({
      key: 'control:next',
      kind: 'next',
      track: null,
      label: `Play ${input.nextLabel}`,
      detail: 'The next episode in this series.',
      selected: false,
    })
  }

  for (const row of buildTrackMenu(input.tracks, input.state)) {
    rows.push({
      key: row.key,
      kind: row.kind === 'off' ? 'off' : row.kind,
      track: row.track,
      label: row.label,
      detail: row.detail,
      selected: row.selected,
    })
  }

  rows.push({
    key: 'control:exit',
    kind: 'exit',
    track: null,
    label: 'Back to the library',
    detail: 'Stops playback. Your position is kept.',
    selected: false,
  })
  return rows
}
