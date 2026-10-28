/**
 * The one place the client's state is assembled.
 *
 * Everything here is decided by a module that has its own tests — the store rules, the
 * progress guard, the focus machine, the host probe, the scan rules — and this file
 * only wires them together and holds the one instance of each. Screens read from it and
 * keep whatever they show in their own reactive state, because Blits re-renders from a
 * component's state and not from a module's.
 *
 * The scan lives here rather than in a screen for the same reason the decoder does: a
 * walk of a USB drive outlives the screen that started it, and a scan that was owned by
 * a component would be abandoned the moment the owner pressed Back.
 */

import { createFocusMemory, type FocusMemory } from '../core/focus.ts'
import { LibraryService, nextId, type StoragePort } from '../core/storage.ts'
import type { LibraryItem, Source } from '../core/contract.ts'
import {
  MAX_ISSUES,
  planCatalog,
  reconcileSources,
  replaceSourceItems,
  withSourceAvailability,
  type ScanIssue,
  type ScanReport,
} from '../core/scan.ts'
import type { DeviceSettings } from '../core/settings.ts'
import { detectHost, readDeviceInfo, type DeviceInfo, type HostCapabilities } from '../platform/env.ts'
import { registerRemoteKeys, type KeyRegistration } from '../platform/keys.ts'
import {
  listMediaRoots,
  watchStorageChanges,
  type MediaRoot,
  type RootsResult,
  type StorageWatch,
} from '../platform/media-roots.ts'
import { walkRoot } from '../platform/scan.ts'
import { createDeviceStorage } from '../platform/persist.ts'
import { recordEvidence } from '../platform/evidence.ts'

/** The library service, one per app run. */
let service: LibraryService | null = null

/** Focus remembered per screen, so Back returns the cursor where it was. */
const focusMemory: FocusMemory = createFocusMemory()

/** The granted sources, keyed by the label the platform reported. */
let roots: readonly MediaRoot[] = []

/** What the host supports, probed once at start. */
let hostCapabilities: HostCapabilities | null = null

/** What the remote accepted, attempted once at start. */
let keyRegistration: KeyRegistration | null = null

/** The device's own identity, as the platform reports it. */
let deviceInfo: DeviceInfo | null = null

/** The storage watch, so a second `prepare()` cannot leave two listeners behind. */
let storageWatch: StorageWatch | null = null

/** True while a walk is running. A second scan is refused rather than queued. */
let scanning = false

/** Set by `cancelScan`; checked between folders by the walker. */
let scanCancelled = false

/** Distinguishes ids generated in the same millisecond. */
let idSequence = 0

/** The host's capabilities, probing on first use. */
export function host(): HostCapabilities {
  hostCapabilities ??= detectHost()
  return hostCapabilities
}

/** What the remote accepted. Empty until `prepare()` has run. */
export function keys(): KeyRegistration {
  return keyRegistration ?? { registered: [], refused: [], supported: false }
}

/** The device identity, or an all-`null` reading on a host that reports none. */
export function device(): DeviceInfo {
  deviceInfo ??= { model: null, firmwareVersion: null, platformVersion: null, productVersion: null }
  return deviceInfo
}

/** The library service, built on first use. */
export function library(): LibraryService {
  service ??= new LibraryService(storagePort())
  return service
}

/** The storage port. A named function so a test or a preview can substitute it. */
function storagePort(): StoragePort {
  if (portOverride !== null) {
    return portOverride
  }
  return createDeviceStorage()
}

let portOverride: StoragePort | null = null

/** Replaces the storage port. Used by the browser preview and by tests. */
export function useStoragePort(port: StoragePort | null): void {
  portOverride = port
  service = null
}

/** Focus remembered per screen. */
export function focus(): FocusMemory {
  return focusMemory
}

/** The drives the TV reported, as of the last refresh. */
export function mediaRoots(): readonly MediaRoot[] {
  return roots
}

/** The catalogued items, joined with the active profile's state. */
export function items(): readonly LibraryItem[] {
  return library().items()
}

/** The granted sources, as stored. */
export function sources(): readonly Source[] {
  return library().state().sources
}

/** The profiles on this device. */
export function profiles() {
  return library().profiles()
}

/** The profile in use, or `null`. */
export function activeProfileId(): string | null {
  return library().activeProfileId()
}

/** This device's playback preferences. */
export function settings(): DeviceSettings {
  return library().settings()
}

/** Stores new playback preferences. False means nothing was written. */
export function saveSettings(next: DeviceSettings): boolean {
  return library().saveSettings(next)
}

/** What the last scan did, as it was stored. */
export function scanReport(): ScanReport {
  return library().lastScan()
}

/** True while a walk is running, for a screen that wants to say so. */
export function scanInProgress(): boolean {
  return scanning
}

/**
 * Probes the host, requests the remote keys, reads the device identity, and starts
 * watching for a drive being plugged in or pulled out.
 *
 * Called once from the app's `ready` hook. Every step is allowed to report failure
 * rather than throw: a TV whose input-device API refuses a key still has arrow keys,
 * an app that refuses to start over a refused key would be worse than one that simply
 * cannot reach the Info button, and a host with no storage-change API still scans when
 * asked.
 */
export function prepare(): void {
  hostCapabilities = detectHost()
  deviceInfo = readDeviceInfo()
  keyRegistration = registerRemoteKeys()
  startStorageWatch()
  recordEvidence('boot', bootDetail())
}

/**
 * The boot facts, as one line.
 *
 * This is the receipt a session that has no screenshot channel can actually read: it
 * says whether the bundle ran at all, which engine ran it, whether the three platform
 * APIs the client depends on were present, how many remote keys the set accepted, and
 * what the set calls itself. A blank screen and a refused bundle look identical from
 * the sofa; after a `sdb pull` they do not.
 */
function bootDetail(): string {
  const caps = host()
  const info = device()
  const keysAccepted = keys()
  const flag = (value: boolean): string => (value ? '1' : '0')
  return [
    `host=${caps.kind}`,
    `avplay=${flag(caps.avplay)}`,
    `filesystem=${flag(caps.filesystem)}`,
    `inputdevice=${flag(caps.inputdevice)}`,
    `storage=${flag(caps.durableStorage)}`,
    `keys=${keysAccepted.registered.length}/refused=${keysAccepted.refused.length}`,
    `model=${info.model ?? 'unknown'}`,
    `firmware=${info.firmwareVersion ?? 'unknown'}`,
    `platform=${info.platformVersion ?? 'unknown'}`,
    `product=${info.productVersion ?? 'unknown'}`,
    `ua=${caps.userAgent}`,
  ].join(' ')
}

/**
 * The storages this app reads, reduced to what reconciling the source list needs.
 *
 * An internal storage and a virtual root (`images`, `documents`, …) are *reported* by the
 * platform and are deliberately not sources: a film does not live on the TV's own flash,
 * and walking it would catalogue the platform's bookkeeping. Settings still lists every
 * storage the platform named, so the choice stays visible.
 */
function scannableMounts(current: readonly MediaRoot[]): { label: string; mounted: boolean }[] {
  return current
    .filter((root) => root.scannable)
    .map((root) => ({ label: root.label, mounted: root.mounted }))
}

/** Asks the TV what storage it has, and remembers the answer. */
export async function refreshMediaRoots(): Promise<RootsResult> {
  const result = await listMediaRoots()
  roots = result.ok ? result.roots : []
  return result
}

/** Grants a discovered drive as a source, unless it is already granted. */
export function grantRoot(root: MediaRoot, resolvedPath: string): Source | null {
  const existing = library()
    .state()
    .sources.find((source) => source.label === root.label)
  if (existing !== undefined) {
    return existing
  }
  const sources: Source[] = [
    ...library().state().sources,
    {
      id: `source-${root.label}`,
      label: root.label,
      rootPath: resolvedPath,
      state: 'available',
      lastScanMs: 0,
    },
  ]
  library().replaceCatalog(library().items(), sources)
  return sources[sources.length - 1] ?? null
}

/** A readable summary of the storage situation, for Settings. */
export function storageSummary(): string {
  if (roots.length === 0) {
    return 'The TV reported no removable storage.'
  }
  return roots
    .map((root) => {
      const state = root.state.toLowerCase()
      const chosen = root.scannable ? 'scanned' : 'not scanned'
      return `${root.label} (${root.type.toLowerCase()}, ${state}, ${chosen})`
    })
    .join(', ')
}

/** True when the app is running on the TV rather than in a browser preview. */
export function onTv(): boolean {
  return host().kind === 'tizen'
}

/**
 * The storage answer, as one line, for the evidence record.
 *
 * The labels are quoted exactly as the set spelled them, because the handoff's open
 * question about drive naming is settled by the label the platform reports and by
 * nothing else. A refused listing carries the platform's own message, which is how a
 * privilege problem is told apart from an empty drive.
 */
function describeRoots(result: RootsResult): string {
  if (!result.ok) {
    return `ok=0 reason=${result.reason}`
  }
  const parts = result.roots.map((root) => {
    const chosen = root.scannable ? 'scanned' : 'skipped'
    return `${root.label || '(no label)'}[${root.type},${root.state},${chosen}]`
  })
  return `ok=1 count=${result.roots.length} ${parts.join(' ') || 'none'}`
}

/** What a finished scan did, as one line, for the evidence record. */
function describeReport(report: ScanReport): string {
  return [
    `sources=${report.scannedSources}`,
    `imported=${report.imported}`,
    `withdrawn=${report.withdrawn}`,
    `complete=${report.complete ? '1' : '0'}`,
    `bounded=${report.bounded ? '1' : '0'}`,
    `issues=${report.issues.length}`,
    `reason=${report.reason ?? 'none'}`,
  ].join(' ')
}

/** Asks the walk to stop at the next folder boundary. */
export function cancelScan(): void {
  scanCancelled = true
}

/**
 * Walks every mounted drive and catalogues what it finds.
 *
 * The order is deliberate and every step has a failure path:
 *
 * 1. **Ask the platform what storage exists.** A storage list that cannot be read
 *    withdraws nothing — an unreadable list is not evidence that a file is gone.
 * 2. **Reconcile the granted sources against what is plugged in.** A drive that is not
 *    mounted is skipped; its rows stay and become unreadable, with their positions
 *    intact.
 * 3. **Walk each mounted source.** A root that will not list is a failed walk, not an
 *    empty drive, and it withdraws nothing either.
 * 4. **Plan one source's catalogue at a time**, carrying ids forward so progress stays
 *    attached, and joining new files to works already in the library.
 * 5. **Write once**: items, sources and the report in a single document, so an
 *    interruption cannot leave half a library update behind.
 *
 * A second scan while one is running returns the stored report rather than starting a
 * second walk, because two walks of one USB drive is how a 2021 TV drops frames.
 */
export async function scanDrives(): Promise<ScanReport> {
  if (scanning) {
    return library().lastScan()
  }
  scanning = true
  scanCancelled = false
  const startedMs = Date.now()
  try {
    const discovered = await refreshMediaRoots()
    recordEvidence('storage', describeRoots(discovered))
    if (!discovered.ok) {
      const report: ScanReport = {
        scannedSources: 0,
        imported: 0,
        withdrawn: 0,
        issues: [],
        complete: false,
        bounded: false,
        finishedMs: startedMs,
        reason: discovered.reason,
      }
      library().recordScan(report)
      recordEvidence('scan', describeReport(report))
      return report
    }

    const reported = discovered.roots
    const sources = reconcileSources(
      library().state().sources,
      scannableMounts(reported),
      // A Tizen storage *is* its location: the platform's own label is what the
      // filesystem API takes, so no mount path is invented here.
      (label) => label,
    )

    let catalog: LibraryItem[] = [...library().items()]
    const issues: ScanIssue[] = []
    const walked = new Set<string>()
    let imported = 0
    let withdrawn = 0
    let scannedSources = 0
    let complete = true
    let bounded = false
    let reason: string | null = null

    for (const source of sources) {
      if (scanCancelled) {
        complete = false
        break
      }
      const mount = reported.find((root) => root.label === source.label)
      if (source.state !== 'available' || mount === undefined || !mount.mounted) {
        // Not plugged in. Nothing is walked and nothing is withdrawn: the rows stay and
        // only their playback is refused.
        catalog = withSourceAvailability(catalog, source.id, false)
        continue
      }

      const walk = await walkRoot({
        root: source.rootPath,
        isCancelled: () => scanCancelled,
      })
      if (walk.rootProblem !== null) {
        // The drive is mounted and will not list. That is a failed walk, so nothing
        // this source held is withdrawn; its rows are marked unreadable and the reason
        // travels in the report.
        catalog = withSourceAvailability(catalog, source.id, false)
        complete = false
        reason = walk.rootProblem
        issues.push(...walk.issues)
        continue
      }

      const plan = planCatalog({
        sourceId: source.id,
        found: walk.files,
        existing: catalog,
        complete: walk.complete,
        nowMs: startedMs,
        newId: (prefix) => nextId(prefix, idSequence++, Date.now()),
      })
      catalog = replaceSourceItems(catalog, source.id, plan.items)
      imported += plan.imported
      withdrawn += plan.withdrawn
      issues.push(...plan.issues)
      scannedSources += 1
      walked.add(source.id)
      complete = complete && walk.complete
      bounded = bounded || walk.bounded
      if (scanCancelled) {
        complete = false
        break
      }
    }

    const finishedMs = Date.now()
    const report: ScanReport = {
      scannedSources,
      imported,
      withdrawn,
      issues: issues.slice(0, MAX_ISSUES),
      complete,
      bounded,
      finishedMs,
      reason,
    }
    const stamped: Source[] = sources.map((source) =>
      walked.has(source.id)
        ? { ...source, state: 'available', lastScanMs: finishedMs }
        : source,
    )
    library().applyScan({ items: catalog, sources: stamped, report })
    recordEvidence('scan', describeReport(report))
    return report
  } finally {
    scanning = false
    scanCancelled = false
  }
}

/**
 * Starts watching for storage changes.
 *
 * Stopping the previous watch first matters: `prepare()` is called once per launch, but
 * a second call would otherwise leave a listener pointing at a screen that no longer
 * exists.
 */
export function startStorageWatch(): StorageWatch {
  stopStorageWatch()
  storageWatch = watchStorageChanges(() => {
    void onStorageChanged()
  })
  return storageWatch
}

/** Stops the storage watch, if one is running. */
export function stopStorageWatch(): void {
  if (storageWatch !== null && storageWatch.watching) {
    storageWatch.stop()
  }
  storageWatch = null
}

/**
 * Marks a source's rows readable or unreadable when a drive arrives or leaves.
 *
 * This is the requirement that a position survives a pulled USB cable, and it is the
 * only thing that happens on the notification: no scan is started behind the owner's
 * back, and no row is ever deleted. A drive coming back therefore needs a scan — which
 * the Settings screen offers — but its history is already there when it returns.
 */
async function onStorageChanged(): Promise<void> {
  const discovered = await refreshMediaRoots()
  if (!discovered.ok) {
    return
  }
  const sources = reconcileSources(
    library().state().sources,
    scannableMounts(discovered.roots),
    (label) => label,
  )
  let catalog: LibraryItem[] = [...library().items()]
  for (const source of sources) {
    // Only a drive that has gone away is acted on. A drive that came back is *not*
    // assumed to still hold every file the last scan saw: its rows stay unreadable
    // until a scan looks, which is the difference between knowing and hoping.
    if (source.state === 'available') {
      continue
    }
    catalog = withSourceAvailability(catalog, source.id, false)
  }
  library().applyScan({ items: catalog, sources, report: library().lastScan() })
}
