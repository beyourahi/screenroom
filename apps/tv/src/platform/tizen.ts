/**
 * The Tizen host objects, declared rather than assumed present.
 *
 * This package typechecks with `"types": []`, so there are no `@types/tizen-*`
 * globals to lean on — and that is deliberate. Every field below is optional and
 * reached through a guarded accessor, so a build that runs in a desktop browser (for
 * a UI preview) cannot accidentally typecheck against an API that is not there, and
 * cannot crash on it either.
 *
 * Only the members this client actually calls are declared. A fuller declaration
 * would be a promise to keep it in sync with a platform that changes yearly, for
 * methods nothing calls.
 *
 * Sources for the shapes: Samsung's Smart TV web API references for
 * `webapis.avplay`, `tizen.filesystem` and `tizen.tvinputdevice`, and the Tizen web
 * Device API reference for `tizen.systeminfo`.
 */

/** A Tizen `systeminfo` property read. */
export interface TizenSystemInfo {
  getPropertyValue(property: string): string | null
}

/** A Tizen `filesystem` storage entry, as `listStorages` reports it. */
export interface TizenStorage {
  /** Platform-dependent label (for example `removable_sda1`). Never hard-code one. */
  readonly label: string
  /** `MOUNTED`, `REMOVED`, `UNMOUNTABLE` and similar, as the platform spells them. */
  readonly state: string
  readonly type: string
}

/** The Tizen filesystem API, limited to what this client uses. */
export interface TizenFilesystem {
  listStorages(
    onSuccess: (storages: TizenStorage[]) => void,
    onError: (error: unknown) => void,
  ): void
}

/** The Tizen remote-key API. Registration is what makes non-arrow keys arrive. */
export interface TizenTvInputDevice {
  registerKey(keyName: string): void
  registerKeyBatch(keyNames: string[]): void
  getSupportedKeys(): readonly { readonly name: string; readonly code: string }[]
}

/**
 * The AVPlay object, as `webapis.avplay`.
 *
 * Every method is invoked only in the states the platform accepts, and the app checks
 * `getState()` first rather than calling and hoping. The type is intentionally loose
 * in the callback signatures: the platform passes extra positional arguments that no
 * caller here needs.
 */
export interface AvPlay {
  open(uri: string): void
  close(): void
  prepareAsync(onSuccess: () => void, onError: (error: unknown) => void): void
  play(): void
  pause(): void
  stop(): void
  seekTo(milliseconds: number, onSuccess?: () => void, onError?: (error: unknown) => void): void
  jumpForward(milliseconds: number): void
  jumpBackward(milliseconds: number): void
  getState(): string
  getCurrentTime(): number
  getDuration(): number
  getTotalTrackInfo(): readonly AvPlayTrack[]
  setSelectTrack(type: 'AUDIO' | 'TEXT', index: number): void
  setDisplayRect(x: number, y: number, width: number, height: number): void
  setDisplayMethod(method: string): void
  setListener(listener: AvPlayListener): void
  suspend(): void
  restore(): void
}

/** One selectable track, as `getTotalTrackInfo` reports it. */
export interface AvPlayTrack {
  readonly type: string
  readonly index: number
  /** A JSON string carrying language and codec details. Parsed defensively. */
  readonly extra_info?: string
}

/** The AVPlay listener callbacks this client installs. */
export interface AvPlayListener {
  onbufferingstart?: () => void
  onbufferingprogress?: (percent: number) => void
  onbufferingcomplete?: () => void
  oncurrentplaytime?: (milliseconds: number) => void
  onstreamcompleted?: () => void
  onerror?: (errorType: string) => void
  onevent?: (eventType: string, eventData: string) => void
  onsubtitlechange?: (duration: number, text: string) => void
}

/** The minimal application API, used only to leave the app on the remote's Back key. */
export interface TizenApplicationManager {
  getCurrentApplication(): { exit(): void }
}

/** The slice of `webapis` this client uses. */
export interface TizenWebApis {
  readonly avplay?: AvPlay
}

/** The Tizen global, with only the used members. */
export interface TizenGlobal {
  readonly webapis?: TizenWebApis
  readonly application?: TizenApplicationManager
  readonly filesystem?: TizenFilesystem
  readonly tvinputdevice?: TizenTvInputDevice
  readonly systeminfo?: TizenSystemInfo
}

/**
 * The Tizen global, or `null` when this is not a Tizen host.
 *
 * Read through `globalThis` rather than as a bare identifier so that a browser
 * preview bundle neither throws a `ReferenceError` nor needs a `typeof` dance at
 * every call site.
 */
export function tizenGlobal(): TizenGlobal | null {
  const candidate = (globalThis as { tizen?: unknown }).tizen
  return typeof candidate === 'object' && candidate !== null ? (candidate as TizenGlobal) : null
}

/** True when this host exposes AVPlay. */
export function hasAvPlay(): boolean {
  return tizenGlobal()?.webapis?.avplay !== undefined
}

/**
 * Leaves the app, as the remote's Back key does from the home screen.
 *
 * Returns whether the host actually exited. On a TV that is the platform's own answer;
 * anywhere else there is no app to leave, and the caller reports that rather than
 * pretending something happened. An app that traps the user on its home screen with a
 * dead Back key is a bug people blame the remote for.
 */
export function exitApp(): boolean {
  const application = tizenGlobal()?.application
  if (application === undefined) {
    return false
  }
  try {
    application.getCurrentApplication().exit()
    return true
  } catch {
    return false
  }
}
