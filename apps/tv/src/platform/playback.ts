/**
 * The playback port, and the two implementations of it.
 *
 * AVPlay is the only playback that counts on the TV. It is also the part of this
 * client that cannot be exercised here at all, so the shape of the port keeps the
 * untestable surface as small as it can be: the port holds no decisions, the state
 * machine it follows is the platform's own documented order, and every arithmetic step
 * it needs lives in `avplay-pure.ts`, where it has tests.
 *
 * The state machine, from Samsung's AVPlay reference:
 *
 * ```text
 * NONE → open(uri) → IDLE → setListener / setDisplayRect / seekTo(start)
 *      → prepareAsync(ok, err) → READY → play() → PLAYING ⇄ pause() → PAUSED
 * stop() → IDLE (configuration kept)      close() → NONE
 * ```
 *
 * Each call is made only in a state the platform accepts, because AVPlay throws
 * otherwise, and a thrown player call inside a key handler takes the whole screen
 * down. `getState()` is therefore checked before every transition rather than
 * assumed.
 *
 * The `<video>` backend exists for one reason: without it the bundle shows nothing in
 * a desktop browser, so no layout or focus work could be previewed at all. It is
 * named, it is reported in Settings, and it must never be counted as a TV result.
 */

import type { TrackChoice } from '../core/tracks.ts'
import { readTracks, toDisplayRect } from './avplay-pure.ts'
import { webapisGlobal, type AvPlay } from './tizen.ts'

/** Which implementation is running. */
export type PlaybackBackend = 'avplay' | 'video'

/** What the player reports back. Every callback may fire more than once. */
export interface PlaybackEvents {
  /** The decoder accepted the file and is ready. */
  onReady(): void
  /** Playback position, in whole milliseconds. Fires often while playing. */
  onPosition(milliseconds: number): void
  /** Duration, in whole milliseconds, once the decoder knows it. */
  onDuration(milliseconds: number): void
  /** The file played to its end. */
  onEnded(): void
  /** Playback failed, with a message that can be shown. */
  onError(message: string): void
  /** Buffering started or finished. */
  onBuffering(active: boolean): void
  /**
   * A subtitle file beside the video was offered to the decoder, and whether it took it.
   *
   * Reported rather than assumed: `setExternalSubtitlePath` failing is silent on some
   * firmware, and a subtitle that never appears is exactly the case where the owner
   * needs to be told the file was refused rather than left guessing.
   */
  onSubtitleFile?(result: { readonly offered: string | null; readonly accepted: boolean }): void
  /**
   * One line of subtitle text the decoder handed over for the app to draw.
   *
   * `onsubtitlechange` exists for *software* subtitles: with an external subtitle file
   * the decoder parses the file and gives the app the text, and the app draws it —
   * which is exactly what Samsung's own subtitle sample does
   * (`subtitlesEl.innerText = text`). An empty string means the line has ended.
   */
  onSubtitleText?(text: string): void
}

/** An open player. Created once per playback, disposed when the user leaves. */
export interface PlaybackSession {
  readonly backend: PlaybackBackend
  /** Starts or resumes playback. */
  play(): void
  /** Pauses. A no-op when not playing, which is what the platform requires. */
  pause(): void
  /** True when the session believes it is paused. */
  isPaused(): boolean
  /** Seeks to an absolute position. */
  seekTo(milliseconds: number): void
  /** Moves relative to the current position. */
  jump(deltaMilliseconds: number): void
  /** The tracks the decoder reports, or an empty list before it is ready. */
  tracks(): readonly TrackChoice[]
  /** Switches an audio or text track. Returns whether the platform accepted it. */
  selectTrack(type: string, index: number): boolean
  /**
   * Hides or shows subtitles without changing which track is selected.
   *
   * Returns whether the decoder accepted the change. This is the platform's own
   * subtitle switch — `setSilentSubtitle` — and not a track index, because a track
   * index cannot express "off" in a way the decoder honours.
   */
  setSubtitlesSilent(silent: boolean): boolean
  /** True when the decoder is currently rendering text silently. */
  subtitlesSilent(): boolean
  /** The last position the decoder reported. */
  positionMs(): number
  /** The duration the decoder reported, or 0. */
  durationMs(): number
  /** The platform's own state string, for the Settings/diagnostics display. */
  state(): string
  /** Releases the decoder and removes the surface it rendered into. */
  dispose(): void
}

/**
 * The rect the video is shown in, and the layer the app draws on top of it.
 *
 * AVPlay renders into its own `<object>`; Lightning renders into a canvas. Both are
 * absolutely positioned children of `<body>`, so which one wins is decided by document
 * order — and the object is created *after* the canvas, which would put it on top and
 * hide every control the app draws. The object is therefore given a lower stacking
 * order than the canvas explicitly, rather than by luck of insertion order.
 */
function createSurface(kind: 'object' | 'video'): HTMLElement {
  const surface = document.createElement(kind === 'object' ? 'object' : 'video')
  if (kind === 'object') {
    surface.setAttribute('type', 'application/avplayer')
  }
  surface.style.position = 'absolute'
  surface.style.top = '0'
  surface.style.left = '0'
  surface.style.width = '100%'
  surface.style.height = '100%'
  surface.style.zIndex = '0'
  surface.style.background = 'transparent'
  document.body.insertBefore(surface, document.body.firstChild)

  const canvas = document.querySelector('canvas')
  if (canvas instanceof HTMLElement) {
    canvas.style.position = 'absolute'
    canvas.style.zIndex = '1'
  }
  return surface
}

/** The viewport the AVPlay rect is expressed against. */
function currentViewport(): { width: number; height: number } {
  const root = document.documentElement
  return { width: root.clientWidth, height: root.clientHeight }
}

/** The full-screen rect, which is the only video layout this slice uses. */
function fullScreenRect(): { x: number; y: number; width: number; height: number } {
  return toDisplayRect(
    { x: 0, y: 0, width: currentViewport().width, height: currentViewport().height },
    currentViewport(),
  )
}

/** A camera-thrown platform error, as a readable message. */
function messageOf(error: unknown): string {
  if (typeof error === 'string') {
    return error
  }
  if (error instanceof Error) {
    return error.message
  }
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const message = (error as { message?: unknown }).message
    if (typeof message === 'string') {
      return message
    }
  }
  return 'The player reported an error without a message.'
}

/** The AVPlay-backed session: the only one that runs on the TV. */
function createAvPlaySession(
  player: AvPlay,
  uri: string,
  startMs: number,
  events: PlaybackEvents,
  options: StartOptions,
): PlaybackSession {
  const surface = createSurface('object')
  let position = 0
  let duration = 0
  let paused = false
  let disposed = false
  let silent = false
  let sidecarAccepted = false
  let tracks: readonly TrackChoice[] = []

  const state = (): string => {
    try {
      return player.getState()
    } catch {
      return 'UNKNOWN'
    }
  }

  /** Reads the decoder's tracks into the app's shape, once the platform allows it. */
  const refreshTracks = (): void => {
    try {
      tracks = readTracks(player.getTotalTrackInfo())
    } catch {
      // `getTotalTrackInfo` is only valid in READY, PLAYING and PAUSED; asking at any
      // other moment throws, and an empty menu is the correct answer then.
      tracks = []
    }
  }

  player.setListener({
    onbufferingstart: () => events.onBuffering(true),
    onbufferingcomplete: () => {
      // The duration is usually known by now and was not at READY.
      try {
        duration = player.getDuration()
      } catch {
        duration = 0
      }
      if (duration > 0) {
        events.onDuration(duration)
      }
      events.onBuffering(false)
    },
    oncurrentplaytime: (milliseconds: number) => {
      if (!Number.isSafeInteger(milliseconds) || milliseconds < 0) {
        return
      }
      position = milliseconds
      events.onPosition(milliseconds)
    },
    onstreamcompleted: () => events.onEnded(),
    onerror: (errorType: string) => events.onError(`AVPlay: ${errorType}`),
    onsubtitlechange: (_duration: number, text: string) => {
      // Only drawn when this app supplied the subtitle file. An embedded text track is
      // rendered by the decoder into its own surface, and drawing the same line again
      // would show it twice — the one bug this rule exists to avoid. Which of the two
      // the decoder reports here is listed in the device handoff as an observable.
      if (sidecarAccepted) {
        events.onSubtitleText?.(typeof text === 'string' ? text : '')
      }
    },
  })

  player.open(uri)
  const rect = fullScreenRect()
  player.setDisplayRect(rect.x, rect.y, rect.width, rect.height)
  // Letter-boxing keeps the file's aspect ratio; stretching to the screen would be a
  // silent picture-quality change nobody asked for.
  player.setDisplayMethod('PLAYER_DISPLAY_MODE_LETTER_BOX')

  // A subtitle file sitting beside the video is offered to the decoder before it
  // prepares, which is the order Samsung's own subtitle sample uses. A refusal is not
  // fatal — the file still plays, and the player screen says the sidecar was not taken
  // rather than showing a subtitle the decoder is not rendering.
  if (options.subtitleFile !== null && options.subtitleFile !== undefined) {
    try {
      player.setExternalSubtitlePath(options.subtitleFile)
      sidecarAccepted = true
    } catch {
      sidecarAccepted = false
    }
  }
  events.onSubtitleFile?.({
    offered: options.subtitleFile ?? null,
    accepted: sidecarAccepted,
  })

  if (startMs > 0) {
    // Valid in IDLE, and the only way to start part-way through: a seek issued after
    // prepare would show the first frame before jumping.
    try {
      player.seekTo(startMs)
      position = startMs
    } catch {
      position = 0
    }
  }

  player.prepareAsync(
    () => {
      refreshTracks()
      try {
        duration = player.getDuration()
      } catch {
        duration = 0
      }
      events.onReady()
      if (duration > 0) {
        events.onDuration(duration)
      }
      player.play()
    },
    (error: unknown) => events.onError(messageOf(error)),
  )

  return {
    backend: 'avplay',
    play(): void {
      if (disposed) {
        return
      }
      const current = state()
      if (current === 'PLAYING' || current === 'PAUSED' || current === 'READY') {
        player.play()
        paused = false
      }
    },
    pause(): void {
      if (disposed) {
        return
      }
      if (state() === 'PLAYING') {
        player.pause()
        paused = true
      }
    },
    isPaused: () => paused,
    seekTo(milliseconds: number): void {
      if (disposed) {
        return
      }
      const current = state()
      if (current === 'PLAYING' || current === 'PAUSED' || current === 'READY') {
        const bounded = Math.max(0, Math.trunc(milliseconds))
        player.seekTo(bounded)
        position = bounded
        events.onPosition(bounded)
      }
    },
    jump(deltaMilliseconds: number): void {
      if (disposed) {
        return
      }
      const current = state()
      if (current !== 'PLAYING' && current !== 'PAUSED') {
        return
      }
      // `jumpForward`/`jumpBackward` are the platform's own relative seek and clamp at
      // the ends themselves, which a seek computed from a stale position would not.
      if (deltaMilliseconds >= 0) {
        player.jumpForward(Math.trunc(deltaMilliseconds))
      } else {
        player.jumpBackward(Math.trunc(-deltaMilliseconds))
      }
    },
    tracks: () => {
      refreshTracks()
      return tracks
    },
    selectTrack(type: string, index: number): boolean {
      if (disposed || (type !== 'AUDIO' && type !== 'TEXT')) {
        return false
      }
      if (!Number.isSafeInteger(index) || index < 0) {
        return false
      }
      try {
        player.setSelectTrack(type, index)
        if (type === 'TEXT') {
          // Choosing a text track is not an instruction to hide it: whatever silent
          // state the decoder was in, a track the owner just picked has to be visible.
          try {
            player.setSilentSubtitle(false)
            silent = false
          } catch {
            // The track was selected; the visibility call is a second attempt, and the
            // menu re-reads the real state on the next open.
          }
        }
        return true
      } catch {
        return false
      }
    },
    setSubtitlesSilent(next: boolean): boolean {
      if (disposed) {
        return false
      }
      try {
        player.setSilentSubtitle(next)
        silent = next
        if (next) {
          // The decoder stops sending lines once it is silent, but the last line it did
          // send is already on screen — clearing it is the difference between
          // "subtitles off" and "subtitles off, with the last one still showing".
          events.onSubtitleText?.('')
        }
        return true
      } catch {
        return false
      }
    },
    subtitlesSilent: () => silent,
    positionMs: () => position,
    durationMs: () => duration,
    state,
    dispose(): void {
      if (disposed) {
        return
      }
      disposed = true
      // `stop()` first, or the last decoded frame stays on screen over the UI;
      // `close()` after, to release the decoder and return to NONE.
      try {
        if (state() === 'PLAYING' || state() === 'PAUSED') {
          player.stop()
        }
      } catch {
        // A decoder that will not stop is already gone; nothing safer to do with it.
      }
      try {
        player.close()
      } catch {
        // Same reasoning: there is no recovery from a failed close, and the decay is
        // reported the next time a session fails to open.
      }
      surface.remove()
    },
  }
}

/**
 * The browser-backed session.
 *
 * This is a preview aid, not a TV path. It exists so the screens can be looked at in a
 * desktop browser, and `hostSummary` says so on screen.
 */
function createVideoSession(uri: string, startMs: number, events: PlaybackEvents): PlaybackSession {
  const element = createSurface('video') as HTMLVideoElement
  let paused = false
  let disposed = false
  let position = startMs
  let duration = 0

  const report = (): void => {
    const reported = Math.round(element.currentTime * 1000)
    if (Number.isSafeInteger(reported) && reported >= 0) {
      position = reported
      events.onPosition(reported)
    }
  }

  element.addEventListener('loadedmetadata', () => {
    duration = Number.isFinite(element.duration) ? Math.round(element.duration * 1000) : 0
    events.onDuration(duration)
    events.onReady()
  })
  element.addEventListener('timeupdate', report)
  element.addEventListener('ended', () => events.onEnded())
  element.addEventListener('waiting', () => events.onBuffering(true))
  element.addEventListener('playing', () => events.onBuffering(false))
  element.addEventListener('error', () =>
    events.onError(`The browser could not play this file (${String(element.error?.code ?? '?')}).`),
  )

  element.src = uri
  if (startMs > 0) {
    // Valid once metadata is loaded; setting it before then is ignored by the browser.
    element.addEventListener(
      'loadedmetadata',
      () => {
        element.currentTime = startMs / 1000
      },
      { once: true },
    )
  }
  void element.play().catch((error: unknown) => events.onError(messageOf(error)))

  return {
    backend: 'video',
    play(): void {
      if (disposed) {
        return
      }
      void element.play()
      paused = false
    },
    pause(): void {
      if (disposed) {
        return
      }
      element.pause()
      paused = true
    },
    isPaused: () => paused,
    seekTo(milliseconds: number): void {
      if (disposed) {
        return
      }
      const bounded = Math.max(0, Math.trunc(milliseconds))
      element.currentTime = bounded / 1000
      position = bounded
      events.onPosition(bounded)
    },
    jump(deltaMilliseconds: number): void {
      if (disposed) {
        return
      }
      const target = Math.min(
        duration > 0 ? duration : Number.MAX_SAFE_INTEGER,
        Math.max(0, position + Math.trunc(deltaMilliseconds)),
      )
      element.currentTime = target / 1000
    },
    // No track switching here: the browser's own controls would be needed to list
    // them, and pretending otherwise would show a menu that does nothing.
    tracks: () => [],
    selectTrack: () => false,
    // The browser has no equivalent of the platform's subtitle switch, and saying so is
    // the point: a preview must not look like a TV that switched subtitles off.
    setSubtitlesSilent: () => false,
    subtitlesSilent: () => false,
    positionMs: () => position,
    durationMs: () => duration,
    state: () => (element.readyState >= 3 ? (paused ? 'PAUSED' : 'PLAYING') : 'IDLE'),
    dispose(): void {
      if (disposed) {
        return
      }
      disposed = true
      element.pause()
      element.removeAttribute('src')
      element.load()
      element.remove()
    },
  }
}

/** What an open may be given besides the file itself. */
export interface StartOptions {
  /** An absolute location for a subtitle file beside the video, or `null`. */
  readonly subtitleFile?: string | null
}

/**
 * Opens a file for playback.
 *
 * The backend is chosen by what the host actually provides, not by a build flag, so a
 * bundle that somehow runs on a TV will use AVPlay even if it was built for preview.
 */
export function startPlayback(
  uri: string,
  startMs: number,
  events: PlaybackEvents,
  options: StartOptions = {},
): PlaybackSession {
  // `webapis` is a sibling of `tizen` on a TV. Reading `tizen.webapis.avplay` returned
  // `undefined` on the owner's set, which silently selected the `<video>` fallback.
  const player = webapisGlobal()?.avplay
  if (player !== undefined) {
    return createAvPlaySession(player, uri, startMs, events, options)
  }
  events.onSubtitleFile?.({
    offered: options.subtitleFile ?? null,
    accepted: false,
  })
  return createVideoSession(uri, startMs, events)
}
