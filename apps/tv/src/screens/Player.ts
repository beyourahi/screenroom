/**
 * Player: one file, on the TV's own decoder.
 *
 * The screen draws and translates key presses; everything about *which* decoder is open
 * and when a position is worth writing lives in `app/player.ts`. That split is what
 * keeps this file about layout instead of about session bookkeeping.
 *
 * Four behaviours are worth knowing before reading on:
 *
 * * the overlay is hidden until the first key press, so playback never begins behind a
 *   caption nobody asked for;
 * * **Up or Down opens the track menu** — audio tracks, the subtitle switch and the
 *   subtitle tracks, in one flat list, which is the same flat index the grids use;
 * * the preferences stored on this device are applied when a file becomes ready, and a
 *   choice made in the menu is remembered for the next file;
 * * a finished episode offers the next one only when the catalogue holds exactly one
 *   candidate one number up. A gap is never skipped, because skipping is how somebody
 *   watches the second half of a story without knowing they missed the first.
 *
 * Leaving always tears the decoder down in the platform's order — the runtime does that
 * in `end()`, and it is called from `destroy` so it happens however the user leaves,
 * including by pressing Back on the remote.
 */

import Blits from '@lightningjs/blits'

import { formatPosition, progressFraction } from '../core/browse.ts'
import { resolveLocator, subtitleStem } from '../core/locator.ts'
import { rememberChoice, type DeviceSettings } from '../core/settings.ts'
import {
  buildTrackMenu,
  chooseTracks,
  nextEpisode,
  sidecarFileNames,
  type TrackChoice,
  type TrackMenuRow,
} from '../core/tracks.ts'
import type { LibraryItem } from '../core/contract.ts'
import { hex, TOKENS } from '../design/tokens.ts'
import { routeBoolean, routeString } from '../app/route-props.ts'
import { items, library, saveSettings, settings } from '../app/runtime.ts'
import * as player from '../app/player.ts'
import { pathExists, fileUri } from '../platform/media-roots.ts'

/** How far the arrow keys seek, in milliseconds. */
const SEEK_STEP_MS = 10_000

/** How far the rewind and fast-forward keys jump, in milliseconds. */
const JUMP_MS = 60_000

/** How long the overlay stays up after the last key press. */
const OVERLAY_MS = 4_000

/** One row of the drawn menu. */
interface MenuRow extends TrackMenuRow {
  readonly y: number
  readonly fill: string
  readonly labelColor: string
  readonly detailColor: string
  readonly marker: string
  readonly detailY: number
}

/**
 * The subtitle file sitting beside a video, or `null`.
 *
 * Only the four extensions Samsung's own player parses are looked for
 * (`core/tracks.ts`), the stem comes from the catalogued name rather than from a guess,
 * and the result is the platform location of the first file that actually exists. A
 * missing sidecar is not an error: most files do not have one.
 */
function findSidecar(absolute: string, relativePath: string): string | null {
  const stem = subtitleStem(relativePath)
  if (stem === null) {
    return null
  }
  const slash = absolute.lastIndexOf('/')
  const directory = slash < 0 ? '' : absolute.slice(0, slash + 1)
  for (const name of sidecarFileNames(stem)) {
    const candidate = `${directory}${name}`
    if (pathExists(candidate) === true) {
      return candidate
    }
  }
  return null
}

/** The last component of a location, for a "subtitles from …" line. */
function fileName(location: string): string {
  const slash = location.lastIndexOf('/')
  return slash < 0 ? location : location.slice(slash + 1)
}

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

      <Text
        :x="$subtitleX"
        :y="$subtitleY"
        content="$subtitleLine"
        size="$subtitleSize"
        color="$foreground"
        align="center"
        maxlines="2"
        wordwrap="true"
        :maxwidth="$subtitleWidth"
      />

      <Element
        :show="$menuOpen"
        :x="$menuX"
        :y="$menuY"
        :w="$menuWidth"
        :h="$menuHeight"
        color="$surface1"
        rounded="$radius"
      >
        <Text
          :x="$menuPad"
          y="0"
          content="$menuTitle"
          size="$menuTitleSize"
          color="$foreground"
          maxlines="1"
          textoverflow="ellipsis"
          :maxwidth="$menuTextWidth"
        />
        <Element
          :for="row in $menuRows"
          key="$row.key"
          x="0"
          :y="$row.y"
          :w="$menuWidth"
          :h="$menuRowHeight"
          :color="$row.fill"
          rounded="$radius"
        >
          <Text
            :x="$menuPad"
            :y="$menuLabelOffset"
            content="$row.label"
            size="$bodySize"
            color="$row.labelColor"
            maxlines="1"
            textoverflow="ellipsis"
            :maxwidth="$menuTextWidth"
          />
          <Text
            :x="$menuPad"
            :y="$row.detailY"
            content="$row.detail"
            size="$captionSize"
            color="$row.detailColor"
            maxlines="1"
            textoverflow="ellipsis"
            :maxwidth="$menuTextWidth"
          />
          <Text
            :x="$menuMarkX"
            :y="$menuLabelOffset"
            content="$row.marker"
            size="$bodySize"
            color="$row.labelColor"
          />
        </Element>
      </Element>
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
      surface1: hex(TOKENS.palette.surface1),
      focusSurface: hex(TOKENS.palette.focusSurface),
      focusForeground: hex(TOKENS.palette.focusForeground),
      pad: Math.round(TOKENS.spacing.shellPad),
      radius: Math.round(TOKENS.radius.surface),
      titleSize: Math.round(TOKENS.type.title),
      bodySize: Math.round(TOKENS.type.body),
      captionSize: Math.round(TOKENS.type.caption),
      subtitleSize: Math.round(TOKENS.type.bodyLg),
      titleY: 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 12),
      subY: 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 9.4),
      barY: 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 6.4),
      barHeight: 8,
      barWidth: 1660,
      barFill: 0,
      footY: 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 3),
      title: '',
      status: '',
      footer: 'Play/pause · |◄ ►| jump a minute · ◄ ► ten seconds · ▲▼ tracks · Back stops',
      /** Kept so the readout can fall back to the catalogued duration. */
      durationMs: 0,
      overlayAlpha: 0,
      scrimAlpha: 0,
      overlayTimer: null as ReturnType<typeof setTimeout> | null,
      /** Software subtitle text, drawn by this app when the decoder hands it over. */
      subtitleLine: '',
      subtitleX: 160,
      subtitleY: 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 16),
      subtitleWidth: 1600,
      /** The track menu. */
      menuOpen: false,
      menuRows: [] as MenuRow[],
      menuIndex: 0,
      menuTitle: '',
      menuX: 0,
      menuY: 0,
      menuWidth: 0,
      menuHeight: 0,
      menuPad: 0,
      menuTitleSize: Math.round(TOKENS.type.bodyLg),
      menuTextWidth: 0,
      menuMarkX: 0,
      menuRowHeight: 0,
      menuLabelOffset: 0,
      /** What the decoder currently has selected, as this screen last set it. */
      audioIndex: null as number | null,
      textIndex: null as number | null,
      silent: false,
      /** The item being played, which is not the route's id once the next episode runs. */
      currentId: '',
      /** The verified next episode, or empty. */
      nextId: '',
      nextLabel: '',
      finished: false,
      /** What happened to a subtitle file beside the video. */
      sidecarNote: '',
    }
  },
  computed: {
    layout() {
      const rowHeight = Math.round(TOKENS.type.body * 3)
      const gap = Math.round(TOKENS.type.caption * 0.5)
      const menuWidth = 820
      return {
        menuWidth,
        menuX: 1920 - menuWidth - Math.round(TOKENS.spacing.shellPad),
        menuY: Math.round(TOKENS.spacing.shellPad * 1.5),
        menuPad: Math.round(TOKENS.spacing.fieldX),
        menuTitleSize: Math.round(TOKENS.type.bodyLg),
        menuTextWidth: menuWidth - Math.round(TOKENS.spacing.fieldX) * 2 - 60,
        menuMarkX: menuWidth - Math.round(TOKENS.spacing.fieldX) - 40,
        menuRowHeight: rowHeight,
        rowGap: rowHeight + gap,
        menuLabelOffset: Math.round(rowHeight * 0.16),
        detailY: Math.round(rowHeight * 0.16 + TOKENS.type.body * 1.25),
        titleBlock: Math.round(TOKENS.type.bodyLg * 2),
      }
    },
  },
  hooks: {
    ready() {
      this.$focus()
      this.open()
    },
    destroy() {
      // However the user leaves — Back, a route change, the app closing — the decoder
      // is closed and the final position is written.
      player.end()
    },
  },
  /**
   * The remote keys, as the engine names them.
   *
   * Back is handled here rather than left to the router, because a Back press must
   * close the track menu before it leaves the file — a menu that swallowed the exit
   * would be as wrong as an exit that lost the menu.
   */
  input: {
    enter() {
      this.enter()
    },
    space() {
      this.playPause()
    },
    play() {
      this.playPause()
    },
    pause() {
      this.playPause()
    },
    playPause() {
      this.playPause()
    },
    left() {
      this.left()
    },
    right() {
      this.right()
    },
    up() {
      this.up()
    },
    down() {
      this.down()
    },
    rewind() {
      this.rewind()
    },
    fastForward() {
      this.fastForward()
    },
    info() {
      this.info()
    },
    back() {
      if (this.menuOpen) {
        this.closeMenu()
        return
      }
      this.$router.back()
    },
  },
  methods: {
    /** The item on screen, from the store rather than from the route. */
    currentItem(): LibraryItem | null {
      const id = this.currentId === '' ? routeString(this.id) : this.currentId
      return items().find((candidate) => candidate.id === id) ?? null
    },
    /** The callbacks a session reports through. */
    events() {
      return {
        onReady: () => {
          this.applyPreferences()
          this.status = this.sidecarNote === '' ? 'Playing.' : this.sidecarNote
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
          this.onEnded()
        },
        onError: (message: string) => {
          this.status = `This file could not be played. ${message}`
        },
        onSubtitleFile: (result: { offered: string | null; accepted: boolean }) => {
          if (result.offered === null) {
            this.sidecarNote = ''
            return
          }
          this.sidecarNote = result.accepted
            ? `Playing with subtitles from ${fileName(result.offered)}.`
            : 'The decoder would not take the subtitle file beside this video.'
        },
        onSubtitleText: (text: string) => {
          this.subtitleLine = text
        },
      }
    },
    /** Resolves the route's file and hands it to the decoder. */
    open() {
      const wantedId = routeString(this.id)
      const item = items().find((candidate) => candidate.id === wantedId)
      if (item === undefined) {
        this.title = 'That file is no longer in the library.'
        this.status = 'Nothing was played. Go back and choose again.'
        return
      }
      const startMs = routeBoolean(this.fromStart, false) ? 0 : item.positionMs
      this.start(item, startMs)
    },
    /**
     * Opens one item at a position.
     *
     * Used both by the route and by the next episode, so the two paths cannot drift
     * apart: a next episode that skipped the path validation would be a way to open a
     * file this app would otherwise refuse.
     */
    start(item: LibraryItem, startMs: number) {
      this.title = item.title
      this.currentId = item.id
      this.subtitleLine = ''
      this.menuOpen = false
      this.finished = false
      this.nextId = ''
      this.nextLabel = ''
      this.sidecarNote = ''
      this.audioIndex = null
      this.textIndex = null
      this.silent = false
      this.durationMs = item.durationMs

      const source = library().source(item.sourceId)
      if (source === null) {
        this.status = 'The drive this file came from is no longer granted. Scan again from Settings.'
        return
      }
      if (source.state !== 'available' || !item.available) {
        this.status = 'That drive is not readable right now. Plug it back in and scan again from Settings.'
        return
      }
      // The stored path is relative and is never trusted as-is: `resolveLocator` refuses
      // anything that would escape the granted root.
      const absolute = resolveLocator(source.rootPath, item.relativePath)
      if (absolute === null) {
        this.status = "This file's stored path is not safe to open, so it was not opened."
        return
      }
      const resolved = fileUri(absolute)
      if (!resolved.ok) {
        this.status = resolved.reason
        return
      }

      const sidecar = findSidecar(absolute, item.relativePath)
      this.status = startMs > 0 ? `Resuming from ${formatPosition(startMs)}…` : 'Starting…'
      player.begin(item, resolved.uri, startMs, this.events(), { subtitleFile: sidecar })
      this.repaint()
    },
    /**
     * Applies this device's preferences to the tracks the decoder just reported.
     *
     * Nothing is selected that the owner did not ask for: an empty preference leaves the
     * decoder's own default in place, and a preference with no matching track selects
     * nothing rather than falling back to the first track in the file.
     */
    applyPreferences() {
      const prefs: DeviceSettings = settings()
      const choice = chooseTracks(player.tracks(), {
        subtitles: prefs.subtitles,
        subtitleLanguage: prefs.subtitleLanguage,
        audioLanguage: prefs.audioLanguage,
      })
      if (choice.selectAudio && choice.audioIndex !== null) {
        if (player.selectTrack('AUDIO', choice.audioIndex)) {
          this.audioIndex = choice.audioIndex
        }
      }
      if (choice.selectText && choice.textIndex !== null) {
        if (player.selectTrack('TEXT', choice.textIndex)) {
          this.textIndex = choice.textIndex
          this.silent = false
          return
        }
      }
      // Subtitles off, or a preference with no track to satisfy it. The platform's own
      // switch is the only way to express "off" that the decoder honours.
      if (player.setSubtitlesSilent(true)) {
        this.silent = true
      }
    },
    /** Rebuilds the readable part of the overlay. */
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
      if (this.menuOpen) {
        return
      }
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
      if (this.menuOpen) {
        return
      }
      this.showOverlay()
      player.jump(-JUMP_MS)
    },
    fastForward() {
      if (this.menuOpen) {
        return
      }
      this.showOverlay()
      player.jump(JUMP_MS)
    },
    left() {
      if (this.menuOpen) {
        return
      }
      this.showOverlay()
      player.seekTo(player.view().positionMs - SEEK_STEP_MS)
    },
    right() {
      if (this.menuOpen) {
        return
      }
      this.showOverlay()
      player.seekTo(player.view().positionMs + SEEK_STEP_MS)
    },
    info() {
      if (this.menuOpen) {
        this.closeMenu()
        return
      }
      if (this.overlayAlpha > 0) {
        this.overlayAlpha = 0
        return
      }
      this.showOverlay()
    },
    /** Up opens the menu, or moves inside it. */
    up() {
      if (this.menuOpen) {
        this.moveMenu(-1)
        return
      }
      this.openMenu()
    },
    /** Down opens the menu, or moves inside it. */
    down() {
      if (this.menuOpen) {
        this.moveMenu(1)
        return
      }
      this.openMenu()
    },
    /** Enter: a menu row, the next episode, or play/pause. */
    enter() {
      if (this.menuOpen) {
        this.chooseMenuRow()
        return
      }
      if (this.finished && this.nextId !== '') {
        this.playNext()
        return
      }
      this.playPause()
    },
    /**
     * Builds and shows the track menu.
     *
     * A file whose decoder reports no tracks gets no menu at all, rather than an empty
     * panel: a control that opens onto nothing is a control that looks broken.
     */
    openMenu() {
      const tracks: readonly TrackChoice[] = player.tracks()
      const rows = buildTrackMenu(tracks, {
        audioIndex: this.audioIndex,
        textIndex: this.textIndex,
        silent: this.silent || player.subtitlesSilent(),
      })
      if (rows.length === 0) {
        this.showOverlay()
        this.status = 'This file reports no audio or subtitle tracks to switch.'
        return
      }
      const layout = this.layout
      // The cursor is clamped *before* the rows are built, so a menu that shrank since
      // last time cannot start with nothing highlighted.
      this.menuIndex = Math.min(this.menuIndex, rows.length - 1)
      this.menuTitle = this.title
      this.menuX = layout.menuX
      this.menuY = layout.menuY
      this.menuWidth = layout.menuWidth
      this.menuPad = layout.menuPad
      this.menuTitleSize = layout.menuTitleSize
      this.menuTextWidth = layout.menuTextWidth
      this.menuMarkX = layout.menuMarkX
      this.menuRowHeight = layout.menuRowHeight
      this.menuLabelOffset = layout.menuLabelOffset
      this.menuRows = rows.map((row, index) => {
        const focused = index === this.menuIndex
        return {
          ...row,
          y: layout.titleBlock + index * layout.rowGap,
          fill: focused ? hex(TOKENS.palette.focusSurface) : hex(TOKENS.palette.surface1),
          labelColor: focused ? hex(TOKENS.palette.focusForeground) : hex(TOKENS.palette.foreground),
          detailColor: focused
            ? hex(TOKENS.palette.focusForeground)
            : hex(TOKENS.palette.inkMuted),
          marker: row.selected ? '●' : '',
          detailY: layout.detailY,
        }
      })
      this.menuHeight =
        layout.titleBlock + rows.length * layout.rowGap + Math.round(TOKENS.spacing.shellPad * 0.5)
      this.menuOpen = true
    },
    /** Moves the menu cursor, clamped to the list. */
    moveMenu(delta: number) {
      if (this.menuRows.length === 0) {
        return
      }
      const next = Math.min(this.menuRows.length - 1, Math.max(0, this.menuIndex + delta))
      if (next === this.menuIndex) {
        return
      }
      this.menuIndex = next
      this.menuRows = this.menuRows.map((row, index) => {
        const focused = index === this.menuIndex
        return {
          ...row,
          fill: focused ? hex(TOKENS.palette.focusSurface) : hex(TOKENS.palette.surface1),
          labelColor: focused ? hex(TOKENS.palette.focusForeground) : hex(TOKENS.palette.foreground),
          detailColor: focused
            ? hex(TOKENS.palette.focusForeground)
            : hex(TOKENS.palette.inkMuted),
        }
      })
    },
    closeMenu() {
      this.menuOpen = false
      this.showOverlay()
    },
    /**
     * Applies the highlighted row.
     *
     * The three cases are the three things the menu can do, and each remembers what the
     * owner chose: an audio language, a subtitle language (which also turns subtitles
     * on), or subtitles off. The menu closes either way, and a refusal is *shown*
     * rather than swallowed — a decoder that will not switch tracks is a fact about the
     * file, not a key press to ignore.
     */
    chooseMenuRow() {
      const row = this.menuRows[this.menuIndex]
      if (row === undefined) {
        this.closeMenu()
        return
      }
      if (row.kind === 'off') {
        const accepted = player.setSubtitlesSilent(true)
        this.silent = accepted
        this.subtitleLine = ''
        saveSettings(rememberChoice(settings(), { kind: 'off' }))
        this.closeMenu()
        this.status = accepted
          ? 'Subtitles off.'
          : 'The decoder would not hide its subtitles for this file.'
        return
      }
      const track = row.track
      if (track === null) {
        this.closeMenu()
        return
      }
      const type = row.kind === 'audio' ? 'AUDIO' : 'TEXT'
      if (!player.selectTrack(type, track.index)) {
        this.closeMenu()
        this.status = 'The decoder refused that track for this file.'
        return
      }
      if (row.kind === 'audio') {
        this.audioIndex = track.index
        saveSettings(rememberChoice(settings(), { kind: 'audio', language: track.language }))
        this.closeMenu()
        this.status = `Audio: ${track.label}.`
        return
      }
      this.textIndex = track.index
      this.silent = false
      saveSettings(rememberChoice(settings(), { kind: 'text', language: track.language }))
      this.closeMenu()
      this.status = `Subtitles: ${track.label}.`
    },
    /**
     * What happens when the file ends.
     *
     * The position is written as finished first, so the resume point is cleared before
     * anything else can touch it. Then one verified next episode is offered — offered,
     * not started: a television that jumps to the next file by itself is a television
     * that decides for someone who may have got up.
     */
    onEnded() {
      player.checkpoint(true)
      this.finished = true
      this.subtitleLine = ''
      const current = this.currentItem()
      const next = current === null ? null : nextEpisode(items(), current)
      if (next === null) {
        this.nextId = ''
        this.nextLabel = ''
        this.status = 'Finished.'
        this.repaint()
        return
      }
      this.nextId = next.id
      const episode =
        next.season !== null && next.episode !== null
          ? `S${String(next.season)} E${String(next.episode)} · `
          : ''
      this.nextLabel = `${episode}${next.title}`
      this.status = `Finished. Enter plays ${this.nextLabel}.`
      this.showOverlay()
      this.repaint()
    },
    /** Opens the offered next episode from the beginning. */
    playNext() {
      const next = items().find((item) => item.id === this.nextId)
      if (next === undefined) {
        this.nextId = ''
        this.status = 'The next episode is no longer in the library.'
        return
      }
      this.start(next, 0)
      this.showOverlay()
    },
  },
})
