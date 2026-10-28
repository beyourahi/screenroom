/**
 * The device's own storage, behind the port `core/storage.ts` defines.
 *
 * A Tizen web widget has no SQLite, so the document lives in `localStorage`. That is
 * allowed by ADR-003 only if the adapter keeps the core's invariants, which is why
 * this file makes no decisions at all: it reads text, writes text, and reports whether
 * the platform accepted the write. Every rule about what the text means lives in
 * `core/store.ts`, where it has tests.
 *
 * The one thing this file does add is honesty about failure. `localStorage.setItem`
 * throws when the quota is exceeded — a real possibility on a 2021 TV — and a full
 * disk must not look like a saved position.
 */

import type { StoragePort } from '../core/storage.ts'

/** Where the document is kept. Namespaced so another widget cannot collide with it. */
export const STORE_KEY = 'screenroom:v1:library'

/** The port backed by the widget's `localStorage`. */
export function createDeviceStorage(): StoragePort {
  return {
    read(): string | null {
      try {
        return globalThis.localStorage.getItem(STORE_KEY)
      } catch {
        // A read that throws is indistinguishable, for our purposes, from a store
        // that has never been written: both produce an empty library, and the
        // service reports that it started fresh rather than refusing to start.
        return null
      }
    },
    write(document: string): boolean {
      try {
        globalThis.localStorage.setItem(STORE_KEY, document)
        return true
      } catch {
        return false
      }
    },
    clear(): void {
      try {
        globalThis.localStorage.removeItem(STORE_KEY)
      } catch {
        // Nothing to report: the caller's next read will show the store is gone.
      }
    },
    describe(): string {
      return 'the TV\'s own storage for this app'
    },
  }
}
