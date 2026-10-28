/**
 * The walk itself: the one part of scanning that has to talk to the TV.
 *
 * It is deliberately small, and everything with a decision in it lives in
 * `core/scan.ts`. What is left here is a queue, three platform calls, and the rules
 * about stopping:
 *
 * * **Bounded.** Entry and depth caps from `core/scan.ts`. A drive that somehow holds a
 *   quarter of a million files stops the walk rather than the app.
 * * **Cancellable.** Checked between folders, so leaving the screen or pulling the drive
 *   stops the work instead of running it out.
 * * **Non-destructive.** There is no write call in this file. A folder that cannot be
 *   read produces an issue and the walk continues; nothing is deleted, moved or modified.
 * * **Yielding.** Every few folders the walk returns to the event loop, so the TV does
 *   not spend a second drawing nothing while a USB drive is enumerated.
 */

import {
  classifyEntry,
  Issues,
  joinEntry,
  MAX_ENTRIES,
  mayDescend,
  type ScanIssue,
  type ScannedFile,
} from '../core/scan.ts'
import { isDirectoryPath, readDirectory, type ReadEntry } from './media-roots.ts'

/** What a walk found. */
export interface WalkOutcome {
  /** Media files, relative to the root that was walked. */
  readonly files: readonly ScannedFile[]
  /** Entries the walk refused, bounded. */
  readonly issues: readonly ScanIssue[]
  /** Entries examined. */
  readonly visited: number
  /**
   * True when the walk saw every entry it could see.
   *
   * This is the flag that decides whether missing files may lose their availability. A
   * walk that stopped at a bound, could not read a folder, or ran out of drive is not
   * complete, and an incomplete walk withdraws nothing.
   */
  readonly complete: boolean
  /** True when the walk hit a bound rather than finishing. */
  readonly bounded: boolean
  /** True when the caller asked it to stop. */
  readonly cancelled: boolean
  /** The reason the root itself could not be read, or `null`. */
  readonly rootProblem: string | null
}

/** How the walk is driven. */
export interface WalkOptions {
  /** The platform location to start from: a storage label, as the TV reported it. */
  readonly root: string
  /** Checked between folders. */
  readonly isCancelled?: () => boolean
  /** Called with running totals, for a screen that wants to show progress. */
  readonly onProgress?: (visited: number, files: number) => void
}

/** One folder still to read. */
interface Pending {
  readonly path: string
  readonly depth: number
}

/** How many folders are read before the walk gives the app a turn. */
const YIELD_EVERY = 20

/** A turn back to the event loop, so the TV keeps drawing while the walk runs. */
function yieldToHost(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0))
}

/**
 * Walks one root.
 *
 * The root is the platform's own storage label. A root it will not list is reported as a
 * root problem and produces no files — importantly, that is *not* the same as "the drive
 * is empty", and the caller treats it as a failed walk rather than a complete one.
 */
export async function walkRoot(options: WalkOptions): Promise<WalkOutcome> {
  const issues = new Issues()
  const files: ScannedFile[] = []
  let visited = 0
  let bounded = false
  let cancelled = false

  const first = await readDirectory(options.root)
  if (!first.ok) {
    return {
      files,
      issues: issues.snapshot(),
      visited,
      complete: false,
      bounded: false,
      cancelled: false,
      rootProblem: first.reason,
    }
  }

  const queue: Pending[] = [{ path: options.root, depth: 0 }]
  // The root's own listing is already in hand; the queue is read from afterwards.
  let rootEntries: readonly ReadEntry[] | null = first.entries
  let foldersRead = 0

  const stop = (): WalkOutcome => ({
    files,
    issues: issues.snapshot(),
    visited,
    complete: false,
    bounded,
    cancelled,
    rootProblem: null,
  })

  while (queue.length > 0) {
    if (options.isCancelled?.() === true) {
      cancelled = true
      return stop()
    }

    const current = queue.shift()
    if (current === undefined) {
      break
    }
    const entries = rootEntries ?? (await readEntries(current.path, issues))
    rootEntries = null

    let reachedBound = false
    for (const entry of entries) {
      if (options.isCancelled?.() === true) {
        cancelled = true
        return stop()
      }
      visited += 1
      if (visited > MAX_ENTRIES) {
        bounded = true
        issues.push('', 'entry_limit')
        reachedBound = true
        break
      }

      const path = joinEntry(current.path, entry.name)
      if (path === current.path) {
        continue
      }
      const name = entry.name.split(/[/\\]/).pop() ?? entry.name
      // The listing's own answer first, then one call per entry only when it did not
      // give one: a `File` object knows what it is, and re-deriving that is a wasted call
      // on a drive with thousands of entries.
      const directory = entry.isDirectory ?? isDirectoryPath(path)
      if (directory === null) {
        // The platform would not say what this is. Reported, not guessed: treating it as
        // a file would catalogue a folder, and treating it as a folder would read a file.
        issues.push(relativeTo(options.root, path), 'unreadable_entry')
        continue
      }
      switch (classifyEntry(name, directory)) {
        case 'media':
          files.push({ relativePath: relativeTo(options.root, path) })
          break
        case 'directory':
          if (mayDescend(current.depth)) {
            queue.push({ path, depth: current.depth + 1 })
          } else {
            issues.push(relativeTo(options.root, path), 'depth_limit')
          }
          break
        case 'downloading':
          issues.push(relativeTo(options.root, path), 'still_downloading')
          break
        case 'hidden_directory':
        case 'ignored':
          break
      }
    }
    if (reachedBound) {
      break
    }

    options.onProgress?.(visited, files.length)
    foldersRead += 1
    if (foldersRead % YIELD_EVERY === 0) {
      await yieldToHost()
    }
  }

  return {
    files,
    issues: issues.snapshot(),
    visited,
    // The core's rule, restated: a bounded walk, or one that raised an issue, saw an
    // unknown amount of the drive, so it may not withdraw anything.
    complete: !bounded && issues.length === 0,
    bounded,
    cancelled: false,
    rootProblem: null,
  }
}

/** Lists one folder, recording the reason when it cannot be read. */
async function readEntries(path: string, issues: Issues): Promise<readonly ReadEntry[]> {
  const result = await readDirectory(path)
  if (result.ok) {
    return result.entries
  }
  issues.push('', looksLikePermission(result.reason) ? 'permission_lost' : 'directory_unavailable')
  return []
}

/** True when a platform message is about permission rather than absence. */
function looksLikePermission(reason: string): boolean {
  const lower = reason.toLowerCase()
  return lower.includes('permission') || lower.includes('denied') || lower.includes('not allowed')
}

/** A platform path, as a path relative to the root that was walked. */
export function relativeTo(root: string, path: string): string {
  const trimmedRoot = root.replace(/[/\\]+$/, '')
  if (trimmedRoot.length === 0) {
    return path.replace(/^[/\\]+/, '')
  }
  const prefix = `${trimmedRoot}/`
  return path.startsWith(prefix) ? path.slice(prefix.length) : path
}
