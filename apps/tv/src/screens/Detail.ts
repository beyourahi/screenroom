/**
 * Detail: one file, and what can be done with it.
 *
 * The actions are a vertical list, so focus is a single index and Up/Down are the only
 * move keys — the same shape the desktop's detail screen has, for the same reason: with
 * one column there is nothing for a left or right press to mean, and giving it a
 * meaning would be inventing navigation the user did not ask for.
 *
 * The two unusual actions are the ones the core thinks are worth having. **Hide this
 * from Continue watching** is per work, not per file, because an owner who does not
 * want the second episode offered does not want the third one either. **Mark as
 * watched** clears the position rather than keeping it, because a finished file with a
 * stored position is a resume point that lies.
 */

import Blits from '@lightningjs/blits'

import { formatPosition, progressFraction } from '../core/browse.ts'
import type { LibraryItem } from '../core/contract.ts'
import { clampIndex } from '../core/focus.ts'
import { hex, TOKENS } from '../design/tokens.ts'
import { routeString } from '../app/route-props.ts'
import { library } from '../app/runtime.ts'

/** One row in the action list. */
interface Action {
  readonly key: string
  readonly label: string
  readonly detail: string
  readonly fill: string
  readonly labelColor: string
  readonly detailColor: string
  readonly focused: boolean
  readonly y: number
}

export default Blits.Component('Detail', {
  props: {
    /** The item's id, passed as route data by the screen that navigated here. */
    id: { required: true },
  },
  template: `
    <Element w="1920" h="1080" color="$background">
      <Text
        x="$pad"
        y="$pad"
        content="$title"
        size="$titleSize"
        color="$foreground"
        maxlines="1"
        textoverflow="ellipsis"
        maxwidth="1660"
      />
      <Text x="$pad" y="$subY" content="$subtitle" size="$bodySize" color="$muted" />
      <Element
        x="$pad"
        :y="$barY"
        w="$barWidth"
        h="$barHeight"
        color="$surface2"
      />
      <Element
        x="$pad"
        :y="$barY"
        :w="$barFill"
        h="$barHeight"
        color="$signal"
      />

      <Element
        :for="action in $actions"
        key="$action.key"
        :x="$pad"
        :y="$action.y"
        w="$listWidth"
        h="$rowHeight"
        :color="$action.fill"
        rounded="$radius"
      >
        <Text
          :x="$fieldX"
          :y="$rowLabelY"
          content="$action.label"
          size="$titleSize"
          color="$action.labelColor"
        />
        <Text
          :x="$fieldX"
          :y="$rowDetailY"
          content="$action.detail"
          size="$captionSize"
          color="$action.detailColor"
          maxlines="1"
          textoverflow="ellipsis"
        />
      </Element>

      <Text x="$pad" y="$footY" content="$footer" size="$captionSize" color="$disabled" />
    </Element>
  `,
  state() {
    return {
      background: hex(TOKENS.palette.background),
      foreground: hex(TOKENS.palette.foreground),
      muted: hex(TOKENS.palette.inkMuted),
      disabled: hex(TOKENS.palette.disabledForeground),
      surface2: hex(TOKENS.palette.surface2),
      signal: hex(TOKENS.palette.signal),
      pad: Math.round(TOKENS.spacing.shellPad),
      radius: Math.round(TOKENS.radius.surface),
      titleSize: Math.round(TOKENS.type.bodyLg),
      bodySize: Math.round(TOKENS.type.body),
      captionSize: Math.round(TOKENS.type.caption),
      subY: Math.round(TOKENS.spacing.shellPad + TOKENS.type.bodyLg * 1.6),
      barY: Math.round(TOKENS.spacing.shellPad + TOKENS.type.bodyLg * 1.6 + TOKENS.type.body * 1.8),
      barHeight: 6,
      barWidth: 1660,
      barFill: 0,
      title: '',
      subtitle: '',
      footer: 'Enter selects · Back returns',
      actions: [] as Action[],
      focusIndex: 0,
    }
  },
  computed: {
    layout() {
      const listTop = Math.round(TOKENS.spacing.shellPad + TOKENS.type.bodyLg * 1.6 + TOKENS.type.body * 4)
      const rowHeight = Math.round(TOKENS.control.lg * 1.6)
      const gap = Math.round(TOKENS.type.caption)
      return {
        listTop,
        rowHeight,
        rowGap: rowHeight + gap,
        listWidth: 1100,
        fieldX: Math.round(TOKENS.spacing.fieldX),
        rowLabelY: Math.round(TOKENS.control.lg * 0.16),
        rowDetailY: Math.round(TOKENS.control.lg * 0.16 + TOKENS.type.bodyLg * 1.4),
        footY: 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 2),
      }
    },
    /** The item this screen is showing, or `null` when the id is not in the store. */
    item() {
      const id = routeString(this.id)
      return library().items().find((candidate) => candidate.id === id) ?? null
    },
  },
  hooks: {
    ready() {
      this.$focus()
      this.reload()
    },
  },
  methods: {
    /** Rebuilds the header and the action list from the store. */
    reload() {
      const item = this.item
      if (item === null) {
        // An id that is not in the store is a real case: a rescan between navigating
        // and arriving removes rows. Saying so beats an empty screen that looks broken.
        this.title = 'That file is no longer in the library.'
        this.subtitle = 'It may have been removed or renamed since the last scan.'
        this.barFill = 0
        this.actions = []
        return
      }
      this.title = item.title
      this.subtitle = this.subtitleFor(item)
      this.barFill = Math.round(this.barWidth * progressFraction(item.positionMs, item.durationMs))
      const workHidden = library().isHidden(item.workId)
      const rows: { kind: string; label: string; detail: string }[] = []
      if (item.positionMs > 0) {
        rows.push({
          kind: 'resume',
          label: `Resume from ${formatPosition(item.positionMs)}`,
          detail: 'Picks up where you stopped.',
        })
      }
      rows.push({
        kind: 'start',
        label: 'Play from the beginning',
        detail: item.positionMs > 0 ? 'Clears the saved position.' : 'Starts this file.',
      })
      rows.push({
        kind: 'watched',
        label: item.watched ? 'Mark as not watched' : 'Mark as watched',
        detail: item.watched
          ? 'Puts it back in Continue watching by clearing the finished flag.'
          : 'Clears the saved position and stops offering it.',
      })
      rows.push({
        kind: 'hide',
        label: workHidden ? 'Show in Continue watching again' : 'Hide from Continue watching',
        detail: workHidden
          ? 'This work will be offered again when you next play it.'
          : 'Applies to every file in this work, not just this one.',
      })
      rows.push({
        kind: 'back',
        label: 'Back to the library',
        detail: 'Leaves this screen. Nothing is saved by leaving.',
      })

      const layout = this.layout
      this.actions = rows.map((row, index) => {
        const focused = index === this.focusIndex
        return {
          key: row.kind,
          label: row.label,
          detail: row.detail,
          fill: focused ? hex(TOKENS.palette.focusSurface) : hex(TOKENS.palette.surface1),
          labelColor: focused ? hex(TOKENS.palette.focusForeground) : hex(TOKENS.palette.foreground),
          detailColor: focused ? hex(TOKENS.palette.focusForeground) : hex(TOKENS.palette.inkMuted),
          focused,
          y: layout.listTop + index * layout.rowGap,
        }
      })
      this.focusIndex = clampIndex(this.focusIndex, this.actions.length)
    },
    subtitleFor(item: LibraryItem): string {
      const parts: string[] = []
      if (item.kind === 'series' && item.season !== null && item.episode !== null) {
        parts.push(`Season ${item.season}, episode ${item.episode}`)
      }
      if (item.year !== null) {
        parts.push(String(item.year))
      }
      if (item.positionMs > 0 && item.durationMs > 0) {
        parts.push(
          `${formatPosition(item.positionMs)} of ${formatPosition(item.durationMs)}`,
        )
      } else if (item.watched) {
        parts.push('Watched')
      } else {
        parts.push('Not started')
      }
      if (!item.available) {
        parts.push('not readable right now')
      }
      return parts.join(' · ')
    },
    move(direction: 'up' | 'down') {
      if (this.actions.length === 0) {
        return
      }
      const current = this.focusIndex
      const next = direction === 'up' ? current - 1 : current + 1
      if (next < 0 || next >= this.actions.length) {
        return
      }
      this.focusIndex = next
      this.markFocus()
    },
    /** Only the fill and ink change when the cursor moves, so only those are rebuilt. */
    markFocus() {
      const layout = this.layout
      this.actions = this.actions.map((action, index) => {
        const focused = index === this.focusIndex
        return {
          ...action,
          focused,
          y: layout.listTop + index * layout.rowGap,
          fill: focused ? hex(TOKENS.palette.focusSurface) : hex(TOKENS.palette.surface1),
          labelColor: focused ? hex(TOKENS.palette.focusForeground) : hex(TOKENS.palette.foreground),
          detailColor: focused ? hex(TOKENS.palette.focusForeground) : hex(TOKENS.palette.inkMuted),
        }
      })
    },
    enter() {
      const action = this.actions[this.focusIndex]
      const item = this.item
      if (action === undefined || item === null) {
        return
      }
      switch (action.key) {
        case 'resume':
          this.$router.to('/player', { id: item.id, fromStart: false })
          return
        case 'start':
          this.$router.to('/player', { id: item.id, fromStart: true })
          return
        case 'watched':
          library().setWatched(item.id, !item.watched)
          this.reload()
          return
        case 'hide':
          library().setResumeHidden(item.workId, !library().isHidden(item.workId))
          this.reload()
          return
        case 'back':
          this.$router.back()
          return
      }
    },
    up() {
      this.move('up')
    },
    down() {
      this.move('down')
    },
  },
})
