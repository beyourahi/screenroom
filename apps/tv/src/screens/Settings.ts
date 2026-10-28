/**
 * Settings: what this device is, and what it can do.
 *
 * This screen is mostly diagnostic, and that is on purpose. The gap this project has to
 * close before it can claim a TV milestone is evidence, and evidence starts with the
 * app being able to say what it is running on: the model the platform reports, which
 * playback path is live, which remote keys were actually accepted, and where state is
 * stored. Every line here is read from the host at the moment the screen opens; nothing
 * is a constant that could go stale and nothing is rounded up into a claim.
 *
 * The one action is **Look for drives again**, and it takes no other action on the
 * owner's behalf: a scan that fails leaves the library untouched, because losing the
 * catalogue is not a reasonable price for a failed walk.
 */

import Blits from '@lightningjs/blits'

import { clampIndex } from '../core/focus.ts'
import { hex, TOKENS } from '../design/tokens.ts'
import {
  device,
  host,
  host as hostReport,
  keys,
  library,
  refreshMediaRoots,
  storageSummary,
} from '../app/runtime.ts'
import { hostSummary } from '../platform/env.ts'

/** One row: a label and what it says. */
interface Row {
  readonly key: string
  readonly label: string
  readonly value: string
  readonly y: number
  readonly fill: string
  readonly labelColor: string
  readonly valueColor: string
  readonly focused: boolean
  /** True for the one row Enter acts on. */
  readonly actionable: boolean
}

export default Blits.Component('Settings', {
  template: `
    <Element w="1920" h="1080" color="$background">
      <Text x="$pad" y="$pad" content="Settings" size="$titleSize" color="$foreground" />
      <Text x="$pad" y="$subY" content="$subtitle" size="$bodySize" color="$muted" />

      <Element
        :for="row in $rows"
        key="$row.key"
        :x="$pad"
        :y="$row.y"
        :w="$listWidth"
        h="$rowHeight"
        :color="$row.fill"
        rounded="$radius"
      >
        <Text
          :x="$fieldX"
          :y="$rowLabelY"
          content="$row.label"
          size="$bodySize"
          color="$row.labelColor"
        />
        <Text
          :x="$valueX"
          :y="$rowLabelY"
          content="$row.value"
          size="$captionSize"
          color="$row.valueColor"
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
      pad: Math.round(TOKENS.spacing.shellPad),
      radius: Math.round(TOKENS.radius.surface),
      titleSize: Math.round(TOKENS.type.title),
      bodySize: Math.round(TOKENS.type.body),
      captionSize: Math.round(TOKENS.type.caption),
      subY: Math.round(TOKENS.spacing.shellPad + TOKENS.type.title * 1.4),
      subtitle: '',
      footer: '',
      listWidth: 1720,
      rowHeight: 0,
      fieldX: 0,
      valueX: 0,
      rowLabelY: 0,
      footY: 0,
      rows: [] as Row[],
      focusIndex: 0,
      busy: false,
    }
  },
  computed: {
    layout() {
      const rowHeight = Math.round(TOKENS.type.body * 2.1)
      const gap = Math.round(TOKENS.type.caption * 0.6)
      return {
        top: Math.round(TOKENS.spacing.shellPad + TOKENS.type.title * 1.4 + TOKENS.type.body * 3),
        rowHeight,
        rowGap: rowHeight + gap,
        fieldX: Math.round(TOKENS.spacing.fieldX),
        valueX: 620,
        rowLabelY: Math.round(rowHeight * 0.3),
        footY: 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 2),
      }
    },
  },
  hooks: {
    ready() {
      this.$focus()
      this.applyLayout()
      this.reload()
    },
  },
  methods: {
    applyLayout() {
      const layout = this.layout
      this.rowHeight = layout.rowHeight
      this.fieldX = layout.fieldX
      this.valueX = layout.valueX
      this.rowLabelY = layout.rowLabelY
      this.footY = layout.footY
    },
    /** Reads every host fact again and rebuilds the list. */
    reload() {
      const capabilities = host()
      const info = device()
      const registration = keys()
      const report = library().loadReport()
      const sourceCount = library().state().sources.length
      const itemCount = library().items().length

      this.subtitle = hostSummary(capabilities)
      const entries: { key: string; label: string; value: string; actionable?: true }[] = [
        { key: 'device', label: 'TV model', value: info.model ?? 'not reported by this host' },
        {
          key: 'firmware',
          label: 'Software version',
          value: info.firmwareVersion ?? 'not reported by this host',
        },
        {
          key: 'platform',
          label: 'Tizen version',
          value: info.platformVersion ?? 'not reported by this host',
        },
        {
          key: 'playback',
          label: 'Playback path',
          value: capabilities.avplay ? 'AVPlay (the TV decoder)' : '<video> element (browser preview)',
        },
        {
          key: 'remote',
          label: 'Remote keys accepted',
          value: registration.supported
            ? `${registration.registered.length} registered, ${registration.refused.length} refused`
            : 'this host has no remote-key API',
        },
        {
          key: 'storage',
          label: 'Where your data is kept',
          value: report.location,
        },
        {
          key: 'durable',
          label: 'Storage writes',
          value: capabilities.durableStorage ? 'accepted' : 'REFUSED — nothing will be saved',
        },
        {
          key: 'lost',
          label: 'State from a previous run',
          value: report.lostStoredState
            ? `not usable: ${report.issues[0]?.reason ?? 'unknown reason'}`
            : 'read without problems',
        },
        {
          key: 'writefail',
          label: 'Last write',
          value: library().writeFailure()?.reason ?? 'accepted',
        },
        { key: 'drives', label: 'Drives the TV reports', value: storageSummary() },
        {
          key: 'sources',
          label: 'Granted and catalogued',
          value: `${sourceCount} ${sourceCount === 1 ? 'source' : 'sources'}, ${itemCount} ${
            itemCount === 1 ? 'file' : 'files'
          }`,
        },
        { key: 'provenance', label: 'Design tokens', value: TOKENS.provenance },
        {
          key: 'rescan',
          label: 'Look for drives again',
          value: this.busy ? 'looking…' : 'press Enter',
          actionable: true,
        },
      ]

      const layout = this.layout
      this.rows = entries.map((entry, index) => {
        const focused = index === this.focusIndex
        return {
          key: entry.key,
          label: entry.label,
          value: entry.value,
          y: layout.top + index * layout.rowGap,
          fill: focused ? hex(TOKENS.palette.focusSurface) : hex(TOKENS.palette.surface1),
          labelColor: focused ? hex(TOKENS.palette.focusForeground) : hex(TOKENS.palette.foreground),
          valueColor: focused ? hex(TOKENS.palette.focusForeground) : hex(TOKENS.palette.inkMuted),
          focused,
          actionable: entry.actionable === true,
        }
      })
      this.focusIndex = clampIndex(this.focusIndex, this.rows.length)
      this.footer = hostReport().kind === 'tizen'
        ? 'Enter acts on the highlighted row · Back returns'
        : 'Browser preview: none of these readings prove TV behaviour'
    },
    /** Rebuilds only the ink, for a cursor move. */
    markFocus() {
      const layout = this.layout
      this.rows = this.rows.map((row, index) => {
        const focused = index === this.focusIndex
        return {
          ...row,
          focused,
          y: layout.top + index * layout.rowGap,
          fill: focused ? hex(TOKENS.palette.focusSurface) : hex(TOKENS.palette.surface1),
          labelColor: focused ? hex(TOKENS.palette.focusForeground) : hex(TOKENS.palette.foreground),
          valueColor: focused ? hex(TOKENS.palette.focusForeground) : hex(TOKENS.palette.inkMuted),
        }
      })
    },
    move(direction: 'up' | 'down') {
      if (this.rows.length === 0) {
        return
      }
      const next = direction === 'up' ? this.focusIndex - 1 : this.focusIndex + 1
      if (next < 0 || next >= this.rows.length) {
        return
      }
      this.focusIndex = next
      this.markFocus()
    },
    /** Enter: the single action this screen offers. */
    async enter() {
      const row = this.rows[this.focusIndex]
      if (row === undefined || !row.actionable || this.busy) {
        return
      }
      this.busy = true
      this.reload()
      await refreshMediaRoots()
      this.busy = false
      this.reload()
    },
    up() {
      this.move('up')
    },
    down() {
      this.move('down')
    },
  },
})
