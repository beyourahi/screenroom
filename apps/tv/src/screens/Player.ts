/**
 * Player: one file, on the TV's own decoder.
 *
 * The screen draws and translates key presses; everything about *which* decoder is open
 * and when a position is worth writing lives in `app/player.ts`. That split is what
 * keeps this file about layout instead of about session bookkeeping.
 *
 * Two behaviours are worth knowing before reading on:
 *
 * * the overlay is hidden until the first key press, so playback never begins behind a
 *   caption nobody asked for;
 * * leaving always tears the decoder down in the platform's order — the runtime does
 *   that in `end()`, and it is called from `destroy` so it happens however the user
 *   leaves, including by pressing Back on the remote.
 *
 * Not done yet, and deliberately not half-built: external subtitle files and an
 * on-screen audio/text track menu. Both need a TV to verify, and a menu that cannot be
 * shown to work is worse than no menu.
 */

import Blits from '@lightningjs/blits'

import { formatPosition, progressFraction } from '../core/browse.ts'
import { hex, TOKENS } from '../design/tokens.ts'
import { routeBoolean, routeString } from '../app/route-props.ts'
import { library } from '../app/runtime.ts'
import * as player from '../app/player.ts'
import { absolutePath, playbackUri } from '../platform/media-roots.ts'

/** How far the arrow keys seek, in milliseconds. */
const SEEK_STEP_MS = 10_000

/** How far the rewind and fast-forward keys jump, in milliseconds. */
const JUMP_MS = 60_000

/** How long the overlay stays up after the last key press. */
const OVERLAY_MS = 4_000

export default Blits.Component('Player', {
  props: {
    id: { required: true },
    /** True when the user asked to start over, which clears the saved position. */
    fromStart: { default: false },
  },
  template: `
    <Element w="1920" h="1080" color="$background" :alpha="$scrimAlpha">
      <Text
        :x="$pad"
        :y="$titleY"
        content="$title"
        size="$titleSize"
        color="$foreground"
        maxlines="1"
        textoverflow="ellipsis"
        maxwidth="1660"
        :alpha="$overlayAlpha"
      />
      <Text
        :x="$pad"
        :y="$subY"
        content="$status"
        size="$bodySize"
        color="$muted"
        maxlines="2"
        wordwrap="true"
        maxwidth="1660"
        :alpha="$overlayAlpha"
      />

      <Element
        :x="$pad"
        :y="$barY"
        :w="$barWidth"
        h="$barHeight"
        color="$trackColor"
        :alpha="$overlayAlpha"
      />
      <Element
        :x="$pad"
        :y="$barY"
        :w="$barFill"
        h="$barHeight"
        color="$signal"
        :alpha="$overlayAlpha"
      />

      <Text
        :x="$pad"
        :y="$footY"
        content="$footer"
        size="$captionSize"
        color="$disabled"
        :alpha="$overlayAlpha"
      />
    </Element>
  `,
  state() {
    return {
      background: hex(TOKENS.palette.background),
      foreground: hex(TOKENS.palette.foreground),
      muted: hex(TOKENS.palette.inkMuted),
      disabled: hex(TOKENS.palette.disabledForeground),
      signal: hex(TOKENS.palette.signal),
      trackColor: hex(TOKENS.palette.surface3),
      pad: Math.round(TOKENS.spacing.shellPad),
      titleSize: Math.round(TOKENS.type.title),
      bodySize: Math.round(TOKENS.type.body),
      captionSize: Math.round(TOKENS.type.caption),
      titleY: 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 12),
      subY: 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 9.4),
      barY: 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 6.4),
      barHeight: 8,
      barWidth: 1660,
      barFill: 0,
      footY: 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 3),
      title: '',
      status: '',
      footer: 'Play/pause · |◄ ►| jump a minute · ◄ ► ten seconds · Back stops and saves',
      /** Kept so the readout can fall back to the catalogued duration. */
      durationMs: 0,
      overlayAlpha: 0,
      scrimAlpha: 0,
      overlayTimer: null as ReturnType<typeof setTimeout> | null,
    }
  },
  hooks: {
    ready() {
      this.open()
    },
    destroy() {
      // However the user leaves — Back, a route change, the app closing — the decoder
      // is closed and the final position is written.
      player.end()
    },
  },
  methods: {
    /** Resolves the file and hands it to the decoder. */
    open() {
      const wantedId = routeString(this.id)
      const item = library()
        .items()
        .find((candidate) => candidate.id === wantedId)
      if (item === undefined) {
        this.title = 'That file is no longer in the library.'
        this.status = 'Nothing was played. Go back and choose again.'
        return
      }
      this.title = item.title

      const source = library().source(item.sourceId)
      if (source === null) {
        this.status =
          'The folder this file came from is no longer granted. Add it again from Settings.'
        return
      }
      // The stored path is relative and is never trusted as-is: `absolutePath` refuses
      // anything that would escape the granted root.
      const absolute = absolutePath(source.rootPath, item.relativePath)
      if (absolute === null) {
        this.status = "This file's stored path is not safe to open, so it was not opened."
        return
      }
      const uri = playbackUri(absolute)
      if (uri === null) {
        this.status = "This file's path could not be turned into something the TV can open."
        return
      }

      const startMs = routeBoolean(this.fromStart, false) ? 0 : item.positionMs
      this.status = startMs > 0 ? `Resuming from ${formatPosition(startMs)}…` : 'Starting…'
      this.durationMs = item.durationMs

      player.begin(item, uri, startMs, {
        onReady: () => {
          this.status = 'Playing.'
          this.repaint()
        },
        onPosition: () => {
          this.repaint()
          player.checkpointIfDue()
        },
        onDuration: (milliseconds: number) => {
          this.durationMs = milliseconds
          this.repaint()
        },
        onBuffering: (active: boolean) => {
          this.status = active ? 'Buffering…' : this.playingLabel()
        },
        onEnded: () => {
          player.checkpoint(true)
          this.status = 'Finished.'
          this.repaint()
        },
        onError: (message: string) => {
          this.status = `This file could not be played. ${message}`
        },
      })
      this.repaint()
    },
    /** The position readout, which is what the overlay is for. */
    repaint() {
      const current = player.view()
      this.durationMs = current.durationMs > 0 ? current.durationMs : this.durationMs
      this.barFill = Math.round(
        this.barWidth * progressFraction(current.positionMs, this.durationMs),
      )
      this.status =
        this.durationMs > 0
          ? `${formatPosition(current.positionMs)} of ${formatPosition(this.durationMs)}` +
            (current.paused ? ' · paused' : '')
          : this.status
    },
    playingLabel(): string {
      return player.view().paused ? 'Paused.' : 'Playing.'
    },
    /** Raises the overlay and arms its own timeout. */
    showOverlay() {
      this.overlayAlpha = 1
      if (this.overlayTimer !== null) {
        this.$clearTimeout(this.overlayTimer)
      }
      this.overlayTimer = this.$setTimeout(() => {
        this.overlayAlpha = 0
        this.overlayTimer = null
      }, OVERLAY_MS)
    },
    playPause() {
      this.showOverlay()
      const current = player.view()
      if (!current.running) {
        return
      }
      if (current.paused) {
        player.resume()
      } else {
        player.pause()
      }
      this.status = this.playingLabel()
      this.repaint()
    },
    rewind() {
      this.showOverlay()
      player.jump(-JUMP_MS)
    },
    fastForward() {
      this.showOverlay()
      player.jump(JUMP_MS)
    },
    left() {
      this.showOverlay()
      player.seekTo(player.view().positionMs - SEEK_STEP_MS)
    },
    right() {
      this.showOverlay()
      player.seekTo(player.view().positionMs + SEEK_STEP_MS)
    },
    info() {
      if (this.overlayAlpha > 0) {
        this.overlayAlpha = 0
        return
      }
      this.showOverlay()
    },
  },
})
