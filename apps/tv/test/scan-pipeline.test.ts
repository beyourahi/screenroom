import { test } from 'node:test'
import assert from 'node:assert/strict'

/**
 * The whole TV scan, against a simulated Tizen filesystem.
 *
 * This is the closest thing to a device test that can run on a laptop, and it is worth
 * being precise about what it proves and what it does not:
 *
 * * **It proves the wiring.** Drives are asked for, walked, planned, stored as one
 *   document, and read back through the same accessors the screens use. Every one of
 *   those steps is the production code; only the platform above it is simulated.
 * * **It does not prove a TV.** A simulated `listDirectory` cannot fail the way a USB
 *   drive on a 2021 television fails, and it says nothing about permissions, timing or
 *   AVPlay. Gate G5 stays blocked until the owner's set is attached.
 *
 * The simulation is deliberately the *documented* shape: `listStorages` reports a label,
 * a type and a state, and `listDirectory` reports entry names.
 */

import { createMemoryStorage } from '../src/core/storage.ts'
import { library, scanDrives, useStoragePort } from '../src/app/runtime.ts'

/** A filesystem as a map from directory location to the names it holds. */
type Tree = Record<string, string[]>

interface FakeFilesystem {
  readonly listStorages: (
    onSuccess: (storages: { label: string; type: string; state: string }[]) => void,
    onError: (error: unknown) => void,
  ) => void
  readonly listDirectory: (
    path: string,
    onSuccess: (entries: string[], path?: string) => void,
    onError?: (error: unknown) => void,
  ) => void
  readonly isDirectory: (path: string) => boolean
  readonly pathExists: (path: string) => boolean
  readonly toURI: (path: string) => string
}

/**
 * Installs a fake Tizen global over the given drives.
 *
 * `deny` names locations whose listing fails with a permission error, which is what a
 * revoked grant looks like from inside the app.
 */
function install(trees: Record<string, Tree>, deny: readonly string[] = []): void {
  const isDir = (location: string): boolean =>
    Object.values(trees).some((tree) => tree[location] !== undefined)
  const exists = (location: string): boolean => {
    if (isDir(location)) {
      return true
    }
    for (const tree of Object.values(trees)) {
      for (const [directory, names] of Object.entries(tree)) {
        for (const name of names) {
          if (`${directory}/${name}` === location && !isDir(`${directory}/${name}`)) {
            return true
          }
        }
      }
    }
    return false
  }
  const filesystem: FakeFilesystem = {
    listStorages: (onSuccess) => {
      const storages = Object.keys(trees).map((label) => ({
        label,
        type: label === 'internal' ? 'INTERNAL' : 'EXTERNAL',
        state: 'MOUNTED',
      }))
      onSuccess(storages)
    },
    listDirectory: (path, onSuccess, onError) => {
      if (deny.includes(path)) {
        onError?.(new Error('Permission denied'))
        return
      }
      for (const tree of Object.values(trees)) {
        const names = tree[path]
        if (names !== undefined) {
          onSuccess(names)
          return
        }
      }
      onError?.(new Error(`No such directory: ${path}`))
    },
    isDirectory: isDir,
    pathExists: exists,
    toURI: (path: string) => `file:///${path}`,
  }
  ;(globalThis as { tizen?: unknown }).tizen = { filesystem }
}

/** A fresh library, so one test's catalogue cannot leak into the next. */
function freshLibrary(): void {
  useStoragePort(createMemoryStorage())
}

const MEDIA_DRIVE: Tree = {
  removable1: ['Movies', 'Shows', 'notes.txt', '.Trashes'],
  'removable1/Movies': ['Dune.2021.2160p.mkv', 'readme.txt'],
  'removable1/Shows': ['Severance.S01E01.mkv', 'Severance.S01E02.mkv', 'Severance.S01E04.mkv'],
  'removable1/.Trashes': ['stale.mkv'],
}

test('a scan catalogues a USB drive and grants it in one pass', async () => {
  freshLibrary()
  install({ removable1: MEDIA_DRIVE })

  const report = await scanDrives()

  assert.equal(report.complete, true)
  assert.equal(report.bounded, false)
  assert.equal(report.scannedSources, 1)
  assert.equal(report.imported, 4, 'four containers, and nothing else')
  assert.deepEqual(
    library()
      .items()
      .map((item) => item.relativePath)
      .sort(),
    ['Movies/Dune.2021.2160p.mkv', 'Shows/Severance.S01E01.mkv', 'Shows/Severance.S01E02.mkv', 'Shows/Severance.S01E04.mkv'],
  )
  // A hidden folder is not descended into, which is what keeps `.Trashes` and its
  // discarded film out of the library.
  assert.equal(
    library()
      .items()
      .some((item) => item.relativePath.includes('.Trashes')),
    false,
  )
  const sources = library().state().sources
  assert.equal(sources.length, 1)
  assert.equal(sources[0]?.label, 'removable1')
  assert.equal(sources[0]?.state, 'available')
  // The label is the location, not a mount path this app invented.
  assert.equal(sources[0]?.rootPath, 'removable1')
  assert.ok((sources[0]?.lastScanMs ?? 0) > 0, 'the source records when it was scanned')
})

test('the parsed identity comes from the filename, not from the folder', async () => {
  freshLibrary()
  install({ removable1: MEDIA_DRIVE })
  await scanDrives()

  const film = library()
    .items()
    .find((item) => item.relativePath.endsWith('Dune.2021.2160p.mkv'))
  assert.equal(film?.kind, 'movie')
  assert.equal(film?.title, 'Dune')
  assert.equal(film?.year, 2021)

  const episode = library()
    .items()
    .find((item) => item.relativePath.endsWith('S01E01.mkv'))
  assert.equal(episode?.kind, 'series')
  assert.equal(episode?.season, 1)
  assert.equal(episode?.episode, 1)
})

test('a rescan keeps every id, so the progress already stored stays attached', async () => {
  freshLibrary()
  install({ removable1: MEDIA_DRIVE })
  await scanDrives()
  const before = new Map(library().items().map((item) => [item.relativePath, item.id]))
  assert.ok(before.size > 0)

  // The drive is scanned again with one more file on it.
  install({
    removable1: {
      ...MEDIA_DRIVE,
      'removable1/Movies': [...(MEDIA_DRIVE['removable1/Movies'] ?? []), 'Arrival.2016.mkv'],
    },
  })
  await scanDrives()

  for (const item of library().items()) {
    if (item.relativePath.endsWith('Arrival.2016.mkv')) {
      continue
    }
    assert.equal(item.id, before.get(item.relativePath), `${item.relativePath} lost its id`)
  }
  assert.equal(library().items().length, 5)
})

test('a file that is gone keeps its row and its position, and loses only playback', async () => {
  freshLibrary()
  install({ removable1: MEDIA_DRIVE })
  await scanDrives()
  const target = library()
    .items()
    .find((item) => item.relativePath.endsWith('Severance.S01E02.mkv'))
  assert.ok(target !== undefined)

  // A position, written the way the player writes one.
  library().beginSession('play-1', target.id)
  library().recordProgress(
    { sessionId: 'play-1', generation: 1 },
    {
      sessionId: 'play-1',
      generation: 1,
      sequence: 1,
      positionMs: 240_000,
      durationMs: 3_000_000,
      ended: false,
      paused: false,
    },
    target.workId,
  )
  assert.equal(library().items().find((item) => item.id === target.id)?.positionMs, 240_000)

  // The file is deleted from the drive, and the drive is scanned again.
  install({
    removable1: {
      ...MEDIA_DRIVE,
      'removable1/Shows': ['Severance.S01E01.mkv', 'Severance.S01E04.mkv'],
    },
  })
  const report = await scanDrives()

  assert.equal(report.withdrawn, 1)
  const after = library().items().find((item) => item.id === target.id)
  assert.ok(after !== undefined, 'the row is kept, never deleted')
  assert.equal(after.available, false)
  assert.equal(after.positionMs, 240_000, 'the position is the owner’s, not the scanner’s')
})

test('a drive that is pulled out becomes disconnected and keeps every position', async () => {
  freshLibrary()
  install({ removable1: MEDIA_DRIVE })
  await scanDrives()
  const count = library().items().length

  // The TV no longer lists the drive at all.
  install({})
  const report = await scanDrives()

  assert.equal(report.scannedSources, 0)
  assert.equal(library().items().length, count, 'nothing is removed when a drive leaves')
  const source = library().state().sources[0]
  assert.equal(source?.state, 'missing', 'a drive the TV does not report is missing')
  assert.equal(
    library()
      .items()
      .every((item) => !item.available),
    true,
    'and every row of it says it cannot be read',
  )
})

test('a drive that comes back is playable again after one scan', async () => {
  freshLibrary()
  install({ removable1: MEDIA_DRIVE })
  await scanDrives()
  install({})
  await scanDrives()
  assert.equal(
    library()
      .items()
      .every((item) => !item.available),
    true,
  )

  install({ removable1: MEDIA_DRIVE })
  await scanDrives()
  assert.equal(library().state().sources[0]?.state, 'available')
  assert.equal(
    library()
      .items()
      .every((item) => item.available),
    true,
  )
})

test('a drive that will not list is reported, and withdraws nothing', async () => {
  freshLibrary()
  install({ removable1: MEDIA_DRIVE })
  await scanDrives()
  const before = library().items().length

  install({ removable1: MEDIA_DRIVE }, ['removable1'])
  const report = await scanDrives()

  assert.equal(report.complete, false)
  assert.equal(report.reason, 'Permission denied')
  assert.equal(report.withdrawn, 0, 'a walk that never ran cannot have withdrawn anything')
  assert.equal(library().items().length, before)
})

test('an internal storage is listed but never walked', async () => {
  freshLibrary()
  install({
    internal: { internal: ['user', 'share'] },
    removable1: { removable1: ['Dune.2021.mkv'] },
  })
  await scanDrives()

  // The TV's own flash is not a film library, and walking it would catalogue the
  // platform's bookkeeping.
  assert.equal(
    library()
      .items()
      .some((item) => item.sourceId.includes('internal')),
    false,
  )
  assert.equal(library().items().length, 1)
  assert.equal(library().state().sources.length, 1)
})

test('the scan report is stored, so Settings can describe it after a restart', async () => {
  freshLibrary()
  install({ removable1: MEDIA_DRIVE })
  const report = await scanDrives()
  assert.deepEqual(library().lastScan(), report)
})

test('a host with no filesystem API says so instead of finding nothing', async () => {
  freshLibrary()
  delete (globalThis as { tizen?: unknown }).tizen
  const report = await scanDrives()

  assert.equal(report.complete, false)
  assert.equal(report.scannedSources, 0)
  assert.match(report.reason ?? '', /no removable-storage API/)
  assert.equal(library().items().length, 0)
})
