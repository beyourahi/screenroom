/**
 * Where the evidence log is kept, behind nothing: `localStorage`, and no decisions.
 *
 * `core/evidence.ts` owns every rule about what an entry is, how long it may be and how
 * the text is shaped. This file only reads and writes the string, exactly as
 * `platform/persist.ts` does for the library — the difference being that a failure here
 * is not worth reporting, because a missing receipt is not a broken app.
 *
 * The key is separate from the library's on purpose. A diagnostic line must never be
 * able to corrupt, resize or replace the document that holds the owner's positions, and
 * a `clear()` of the library must not silently take the record with it.
 */

import {
  appendEvidence,
  decodeEvidence,
  encodeEvidence,
  MAX_ENTRIES,
  type EvidenceEntry,
} from '../core/evidence.ts'

/** Where the record lives. Namespaced like the library, versioned the same way. */
export const EVIDENCE_KEY = 'screenroom:v1:evidence'

/** Reads the record. Missing, blocked or unparseable all mean "nothing recorded". */
export function readEvidence(): EvidenceEntry[] {
  try {
    return decodeEvidence(globalThis.localStorage.getItem(EVIDENCE_KEY))
  } catch {
    return []
  }
}

/**
 * Appends one entry and writes the record back.
 *
 * Best-effort in every direction: a full quota, a blocked write or a missing
 * `localStorage` all end the same way — nothing recorded, nothing thrown. The in-memory
 * log is still returned so a caller can show it on screen even when the TV would not
 * keep it, which is the difference between a receipt and a claim.
 */
export function recordEvidence(
  tag: string,
  detail: string,
  now: () => number = () => Date.now(),
): EvidenceEntry[] {
  const existing = readEvidence()
  const next = appendEvidence(existing, { atMs: now(), tag, detail }, MAX_ENTRIES)
  try {
    globalThis.localStorage.setItem(EVIDENCE_KEY, encodeEvidence(next))
  } catch {
    // Nothing to do: the caller still gets the entry, and the pull will simply be
    // missing it. A TV that will not store a diagnostic line still plays video.
  }
  return next
}

/** Removes the record. Used by tests; no screen offers it, because it proves nothing. */
export function clearEvidence(): void {
  try {
    globalThis.localStorage.removeItem(EVIDENCE_KEY)
  } catch {
    // Nothing to report: a next read shows the record is gone either way.
  }
}
