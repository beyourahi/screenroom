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

/**
 * A Tizen `filesystem` storage entry, as `listStorages` reports it.
 *
 * The three attributes are the whole interface in the Tizen 6.0 TV reference
 * (`FileSystemStorage`): `label`, `type` and `state`. The reference notes that the label
 * is also the *location* to pass to a resolve or a listing call, which is why this
 * client stores the label and never constructs a mount path of its own.
 */
export interface TizenStorage {
  /** Platform-dependent label (for example `removable1`). Never hard-code one. */
  readonly label: string
  /** `MOUNTED`, `REMOVED` or `UNMOUNTABLE`, as the platform spells them. */
  readonly state: string
  /** `INTERNAL` or `EXTERNAL`, as the platform spells them. */
  readonly type: string
}

/**
 * The Tizen filesystem API, limited to what this client uses.
 *
 * Declared from the Tizen 6.0 TV Web Device API reference (`filesystem.html`). The
 * four synchronous predicates — `isFile`, `isDirectory`, `pathExists` and `toURI` —
 * raise on failure rather than reporting it, so every call site wraps them; a thrown
 * platform call inside a key handler takes the screen down.
 *
 * Nothing that writes is declared: no `deleteFile`, no `createDirectory`, no
 * `openFile`. The absence is the point — there is no code path in this client that
 * could modify the owner's media, and a missing declaration is the strongest form of
 * that promise this file can make.
 */
/** A folder handle, as `resolve` yields it: the thing that can list its own contents. */
export interface TizenFileHandle {
  readonly name?: string
  readonly isDirectory?: boolean
  readonly path?: string
  /** Lists this folder's children as `File` objects, per the API reference. */
  listFiles?(onSuccess: (files: unknown) => void, onError?: (error: unknown) => void): void
}

export interface TizenFilesystem {
  listStorages(
    onSuccess: (storages: TizenStorage[]) => void,
    onError: (error: unknown) => void,
  ): void
  /**
   * Turns a virtual location — which is what a storage label is — into a folder handle.
   *
   * The documented first step of listing (W3C Filesystem API, implemented by Tizen); the
   * handle it yields has `listFiles`. Optional because the reader keeps a fallback.
   */
  resolve?(
    location: string,
    onSuccess: (file: TizenFileHandle) => void,
    onError?: (error: unknown) => void,
    mode?: string,
  ): void
  /**
   * Lists one folder, on firmware that ships the device-API spelling.
   *
   * The callback receives an array of `File` objects per the API reference; a name-only
   * array is accepted too, and `readEntriesFrom` normalizes whichever arrives.
   */
  listDirectory?(
    path: string,
    onSuccess: (entries: unknown, path?: string) => void,
    onError?: (error: unknown) => void,
    filter?: unknown,
  ): void
  /** True when the path is a folder. Raises on failure. */
  isDirectory(path: string): boolean
  /** True when the path exists. Raises on failure. */
  pathExists(path: string): boolean
  /** The platform's own URI for a path. Raises on failure. */
  toURI(path: string): string
  /** Registers a storage add/remove/eject notification; returns a watch id. */
  addStorageStateChangeListener?(
    onSuccess: (storage: TizenStorage) => void,
    onError?: (error: unknown) => void,
  ): number
  removeStorageStateChangeListener?(watchId: number): void
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
  /**
   * Silences the subtitle renderer without changing the selected track.
   *
   * This — not a negative track index — is how AVPlay turns subtitles off, and it is
   * what Samsung's own subtitle sample calls when the user toggles them
   * (`setSilentSubtitle(!subtitlesOn)`).
   */
  setSilentSubtitle(silent: boolean): void
  /** Points the decoder at a subtitle file beside the video. */
  setExternalSubtitlePath(path: string): void
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

/**
 * The Samsung product API: what the set calls itself.
 *
 * This is the TV's own identity, and it is reported by `webapis.productinfo` rather
 * than by `tizen.systeminfo` — the system-info property names the reference lists
 * return nothing on a TV web widget. Every member is optional and read through a
 * guard, because the API differs by model year.
 */
export interface TizenProductInfo {
  getModel?(): string
  getModelCode?(): string
  getFirmware?(): string
  getVersion?(): string
  getDuid?(): string
}

/** The slice of `webapis` this client uses. */
export interface TizenWebApis {
  readonly avplay?: AvPlay
  readonly productinfo?: TizenProductInfo
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

/**
 * The `webapis` global, which is where AVPlay and the product API live.
 *
 * **This is a sibling of `tizen`, not a member of it.** `$WEBAPIS/webapis/webapis.js`
 * installs `window.webapis`, while the device APIs live on `window.tizen`; reading
 * `tizen.webapis.avplay` therefore returns `undefined` on every real television, and
 * the client concluded it was running in a browser and fell back to a `<video>`
 * element. That is what the first installed build reported from the TV itself
 * (`avplay=0` with `filesystem=1`), and it would have meant no local playback at all.
 * The nested form is still accepted as a fallback for hosts that expose it that way.
 */
export function webapisGlobal(): TizenWebApis | null {
  const candidate = (globalThis as { webapis?: unknown }).webapis
  if (typeof candidate === 'object' && candidate !== null) {
    return candidate as TizenWebApis
  }
  return tizenGlobal()?.webapis ?? null
}

/** True when this host exposes AVPlay. */
export function hasAvPlay(): boolean {
  return webapisGlobal()?.avplay !== undefined
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
