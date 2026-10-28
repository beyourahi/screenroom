/**
 * The one place the client's state is assembled.
 *
 * Everything here is decided by a module that has its own tests — the store rules, the
 * progress guard, the focus machine, the host probe — and this file only wires them
 * together and holds the one instance of each. Screens read from it and keep whatever
 * they show in their own reactive state, because Blits re-renders from a component's
 * state and not from a module's.
 */

import { createFocusMemory, type FocusMemory } from '../core/focus.ts'
import { LibraryService, type StoragePort } from '../core/storage.ts'
import type { LibraryItem, Source } from '../core/contract.ts'
import { detectHost, readDeviceInfo, type DeviceInfo, type HostCapabilities } from '../platform/env.ts'
import { registerRemoteKeys, type KeyRegistration } from '../platform/keys.ts'
import { listMediaRoots, type MediaRoot } from '../platform/media-roots.ts'
import { createDeviceStorage } from '../platform/persist.ts'

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
  deviceInfo ??= { model: null, firmwareVersion: null, platformVersion: null }
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

/** The profiles on this device. */
export function profiles() {
  return library().profiles()
}

/** The profile in use, or `null`. */
export function activeProfileId(): string | null {
  return library().activeProfileId()
}

/**
 * Probes the host, requests the remote keys and reads the device identity.
 *
 * Called once from the app's `ready` hook. Every step is allowed to report failure
 * rather than throw: a TV whose input-device API refuses a key still has arrow keys,
 * and an app that refuses to start over a refused key would be worse than one that
 * simply cannot reach the Info button.
 */
export function prepare(): void {
  hostCapabilities = detectHost()
  deviceInfo = readDeviceInfo()
  keyRegistration = registerRemoteKeys()
}

/** Asks the TV what storage it has, and remembers the answer. */
export async function refreshMediaRoots(): Promise<readonly MediaRoot[]> {
  const result = await listMediaRoots()
  roots = result.ok ? result.roots : []
  return roots
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
    .map((root) => `${root.label} (${root.state.toLowerCase()})`)
    .join(', ')
}

/** True when the app is running on the TV rather than in a browser preview. */
export function onTv(): boolean {
  return host().kind === 'tizen'
}
