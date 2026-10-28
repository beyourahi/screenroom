/**
 * The D-pad focus model, as arithmetic.
 *
 * Focus on a TV has to be deterministic and has to come back where it was, and both of
 * those are properties of a small state machine rather than of a renderer. Keeping the
 * machine here means the rules are tested without a TV, and each component only has to
 * draw whatever this says is focused.
 *
 * The rules are the desktop's, deliberately and exactly (`App::move_focus` in
 * `apps/desktop/src/app.rs`), because the same owner uses both devices and an arrow
 * key that behaves differently on the sofa than at the desk is a bug people blame
 * themselves for:
 *
 * * The cursor is a **flat index** into the visible list, not a row/column pair.
 *   Left and right move one item, which means they cross a row boundary at the ends
 *   of a row rather than stopping there.
 * * Up and down move a whole row, using the **drawn** column count, so a layout change
 *   cannot make an arrow press land somewhere other than the cell it was aimed at.
 * * A press that would leave the list does nothing at all. It never wraps and never
 *   clamps sideways: an earlier version of this file clamped `up` from the first row
 *   to index 0, which moved the cursor sideways when the user pressed up.
 */

/** A direction a D-pad can press. */
export type Direction = 'left' | 'right' | 'up' | 'down'

/** Clamps an index into `0..count`, or -1 when there is nothing to focus. */
export function clampIndex(index: number, count: number): number {
  if (count <= 0) {
    return -1
  }
  if (!Number.isFinite(index)) {
    return 0
  }
  const whole = Math.trunc(index)
  if (whole < 0) {
    return 0
  }
  return whole > count - 1 ? count - 1 : whole
}

/**
 * The index one press moves to.
 *
 * `columns` is the count the grid was actually drawn with. A non-positive or
 * non-finite value is treated as one column, which degrades to a list rather than
 * producing `NaN` indices.
 */
export function moveFocus(
  index: number,
  count: number,
  columns: number,
  direction: Direction,
): number {
  if (count <= 0) {
    return -1
  }
  const current = clampIndex(index, count)
  const stride = Number.isFinite(columns) && columns >= 1 ? Math.trunc(columns) : 1

  switch (direction) {
    case 'left':
      return current > 0 ? current - 1 : current
    case 'right':
      return current + 1 < count ? current + 1 : current
    case 'up':
      return current - stride >= 0 ? current - stride : current
    case 'down':
      return current + stride < count ? current + stride : current
  }
}

/**
 * Focus remembered per screen.
 *
 * A TV user presses Back, and expects to be where they were, not at the top of the
 * screen again. The key is the screen's route, so a category screen that is opened
 * for a different source does not inherit the previous source's index.
 */
export interface FocusMemory {
  /** The remembered index for a key, or `undefined` when nothing is remembered. */
  get(key: string): number | undefined
  /** Remembers an index, clamped to the items that were there when it was set. */
  set(key: string, index: number, count: number): void
  /** Forgets a key, for when its screen is gone for good. */
  clear(key: string): void
}

/** An in-memory focus memory. */
export function createFocusMemory(): FocusMemory {
  const remembered = new Map<string, number>()
  return {
    get(key: string): number | undefined {
      return remembered.get(key)
    },
    set(key: string, index: number, count: number): void {
      remembered.set(key, clampIndex(index, count))
    },
    clear(key: string): void {
      remembered.delete(key)
    },
  }
}

/**
 * The index a screen should start on.
 *
 * Falls back to the first item when there is nothing remembered, and to the last
 * item when what was remembered is now past the end — which is what happens when a
 * rescan removes files while the user is in another screen. Landing on the last
 * remaining item is better than landing on nothing.
 */
export function restoreFocus(memory: FocusMemory, key: string, count: number): number {
  if (count <= 0) {
    return -1
  }
  const remembered = memory.get(key)
  if (remembered === undefined) {
    return 0
  }
  return clampIndex(remembered, count)
}
