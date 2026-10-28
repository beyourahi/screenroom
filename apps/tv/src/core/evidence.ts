/**
 * The device's own account of what happened, kept where the owner's library is not.
 *
 * This exists because of a fact about a retail Samsung TV rather than a preference: the
 * set exposes `install`, `launch`, `applist` and `getduid` over SDB and *nothing else*.
 * There is no screenshot channel, no `dlog` over the network, no remote-web-inspector
 * port unless the app was launched in debug mode through a tool this firmware does not
 * ship, and no way to inject a key press. So a session that installs the app can see
 * that it launched and cannot see a single thing after that.
 *
 * The one channel left is the app's own storage: `localStorage` is persisted inside the
 * widget's data directory, and `sdb pull` can read that directory off the television. A
 * bounded log written there turns "it launched" into "the engine ran, it reported this
 * user agent, these keys were accepted, these drives were visible, and the scan did
 * this" — evidence that needed no remote control and no screenshot.
 *
 * Three rules keep it honest and cheap:
 *
 * * **It is not the library.** A separate key, so a diagnostic line can never corrupt
 *   or bloat the document that holds positions.
 * * **It is bounded.** A ring of the last `MAX_ENTRIES`, each detail cut to
 *   `MAX_DETAIL`. A TV's storage is small and synchronous, and a log that grows without
 *   limit is a way to lose the library it was meant to protect.
 * * **It never throws.** Recording is best-effort by definition: a device that refuses
 *   the write has producing no receipt, not a crash.
 *
 * The encoding is deliberately trivial — one entry per line, tab-separated — so that a
 * receipt can be read with `cat` after a `sdb pull` instead of needing this app to
 * interpret it.
 */

/** One thing that happened, with the time it happened. */
export interface EvidenceEntry {
  /** Milliseconds since the epoch, from the device's own clock. */
  readonly atMs: number
  /** A short stable name for the kind of event, e.g. `boot` or `scan`. */
  readonly tag: string
  /** The detail, already flattened to one line. */
  readonly detail: string
}

/** How many entries are kept. Older ones are dropped first. */
export const MAX_ENTRIES = 60

/** The longest one detail may be, before it is cut. */
export const MAX_DETAIL = 300

/** The longest a tag may be. A tag is a name, not a sentence. */
export const MAX_TAG = 24

/**
 * Flattens text to a single bounded field.
 *
 * Tabs and newlines are the record separator, so a detail that contains one would
 * silently split into a second entry — a log that lies is worse than a truncated one.
 * Control characters are the same problem in another costume, so they go too.
 */
export function sanitise(text: string, max: number = MAX_DETAIL): string {
  const flat = text
    .replace(/[\t\n\r]+/g, ' ')
    // Anything else in the C0 range, plus DEL. Written as a range test rather than a
    // regex literal so the intent survives a copy into a terminal.
    .split('')
    .filter((character) => {
      const code = character.charCodeAt(0)
      return code >= 0x20 && code !== 0x7f
    })
    .join('')
    .trim()
  if (flat.length <= max) {
    return flat
  }
  return `${flat.slice(0, Math.max(0, max - 1))}…`
}

/**
 * Appends an entry, keeping the log bounded.
 *
 * A timestamp that is not a finite number is replaced rather than trusted: `Date.now()`
 * on a device with a wrong clock can be `NaN` after arithmetic, and `NaN` in the first
 * column would make every later line unparseable by a reader that splits on tabs.
 */
export function appendEvidence(
  log: readonly EvidenceEntry[],
  entry: EvidenceEntry,
  limit: number = MAX_ENTRIES,
): EvidenceEntry[] {
  const atMs = Number.isFinite(entry.atMs) ? Math.max(0, Math.trunc(entry.atMs)) : 0
  const tag = sanitise(entry.tag, MAX_TAG)
  if (tag.length === 0) {
    return [...log]
  }
  const next = [...log, { atMs, tag, detail: sanitise(entry.detail) }]
  if (limit <= 0) {
    return []
  }
  return next.length > limit ? next.slice(next.length - limit) : next
}

/** Serialises the log to the text that storage keeps. */
export function encodeEvidence(log: readonly EvidenceEntry[]): string {
  return log.map((entry) => `${entry.atMs}\t${entry.tag}\t${entry.detail}`).join('\n')
}

/**
 * Parses stored text back into entries.
 *
 * Every line is validated on its own and a line that does not fit is dropped: this is
 * read after a firmware update, a half-finished write or a pull from a device whose
 * storage was truncated, and one damaged line must not lose the rest of the record.
 * `null` and empty text mean "nothing was ever recorded", which is not an error.
 */
export function decodeEvidence(text: string | null): EvidenceEntry[] {
  if (text === null || text.length === 0) {
    return []
  }
  const entries: EvidenceEntry[] = []
  for (const line of text.split('\n')) {
    const firstTab = line.indexOf('\t')
    const secondTab = firstTab < 0 ? -1 : line.indexOf('\t', firstTab + 1)
    if (secondTab < 0) {
      continue
    }
    const stamp = Number(line.slice(0, firstTab))
    const tag = line.slice(firstTab + 1, secondTab)
    if (!Number.isFinite(stamp) || stamp < 0 || tag.length === 0) {
      continue
    }
    entries.push({
      atMs: Math.trunc(stamp),
      tag: sanitise(tag, MAX_TAG),
      detail: sanitise(line.slice(secondTab + 1)),
    })
  }
  return entries
}

/** A one-line summary of the log, for a screen that has no room for the whole thing. */
export function summariseEvidence(log: readonly EvidenceEntry[]): string {
  if (log.length === 0) {
    return 'Nothing has been recorded on this device yet.'
  }
  const last = log[log.length - 1]
  if (last === undefined) {
    return 'Nothing has been recorded on this device yet.'
  }
  return `${log.length} entries, last ${last.tag}: ${last.detail}`
}
