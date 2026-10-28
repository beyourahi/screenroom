/**
 * Reading the values a route hands to a screen.
 *
 * Blits declares a prop as `{ required: true }`, and its published types map that
 * definition object straight onto `this` — so `this.id` is typed as the *definition*
 * rather than as the string it is at runtime. That is a gap in the framework's types,
 * not something this app can fix, and these two helpers are the one place it is
 * bridged.
 *
 * They are worth more than a cast, though, because they also validate. Route data is
 * built from values the app put there, but a route can also be entered with no data at
 * all — a deep link, a Back into a screen whose item has since been removed — and a
 * screen that reads `undefined` as an item id would ask the store for an item called
 * `undefined`. These return the fallback instead, and the screen then says the honest
 * thing ("that file is no longer in the library") rather than searching for a ghost.
 */

/** A route prop that should be a string, or the fallback. */
export function routeString(value: unknown, fallback = ''): string {
  return typeof value === 'string' ? value : fallback
}

/** A route prop that should be a boolean, or the fallback. */
export function routeBoolean(value: unknown, fallback: boolean): boolean {
  return typeof value === 'boolean' ? value : fallback
}
