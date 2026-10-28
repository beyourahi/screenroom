/**
 * Settings: what this device is, what it can do, and the two things worth pressing.
 *
 * This screen is mostly diagnostic, and that is on purpose. The gap this project has to
 * close before it can claim a TV milestone is evidence, and evidence starts with the
 * app being able to say what it is running on: the model the platform reports, which
 * playback path is live, which remote keys were actually accepted, where state is
 * stored, which drives the TV reports, and what the last scan of them actually did.
 * Every line here is read from the host at the moment the screen opens; nothing is a
 * constant that could go stale and nothing is rounded up into a claim.
 *
 * Two actions live here, and neither takes a decision on the owner's behalf:
 *
 * * **Scan for media** walks the mounted drives and catalogues what it finds. A failed
 *   walk withdraws nothing — losing the catalogue is not a reasonable price for a
 *   failed scan — and the report says which drive would not list and why.
 * * **Subtitles / audio language** are the device's own preferences, in the same
 *   document as the library. Setting one is one small write.
 */

import Blits from '@lightningjs/blits'

import { clampIndex } from '../core/focus.ts'
import { describeScan, type ScanReport } from '../core/scan.ts'
import {
  describeAudio,
  describeSubtitles,
  forgetChoices,
  type DeviceSettings,
} from '../core/settings.ts'
import { hex, TOKENS } from '../design/tokens.ts'
import {
  cancelScan,
  device,
  host,
  keys,
  library,
  mediaRoots,
  onTv,
  refreshMediaRoots,
  saveSettings,
  scanDrives,
  scanInProgress,
  scanReport,
  settings,
  storageSummary,
} from '../app/runtime.ts'
import { hostSummary } from '../platform/env.ts'

/** What Enter does on a row, when it does anything. */
type Action = 'scan' | 'subtitles' | 'audio' | 'forget'

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
  /** True for the rows Enter acts on. */
  readonly actionable: boolean
  /** What Enter does, or `null` for a row that only reports. */
  readonly action: Action | null
}

/** One line of the row list, before it is placed. */
interface Entry {
  readonly key: string
  readonly label: string
  readonly value: string
  readonly action?: Action
}

/** How many drives and sources are listed before the list is summarised. */
const MAX_LISTED = 4

/** Whole milliseconds as a short "how long ago", never as an invented date. */
function sinceMs(timestampMs: number, nowMs: number): string {
  if (timestampMs <= 0) {
    return 'never'
  }
  const elapsed = Math.max(0, nowMs - timestampMs)
  const minutes = Math.floor(elapsed / 60_000)
  if (minutes < 1) {
    return 'just now'
  }
  if (minutes < 60) {
    return `${minutes} min ago`
  }
  const hours = Math.floor(minutes / 60)
  return hours < 24 ? `${hours} h ago` : `${Math.floor(hours / 24)} d ago`
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
      /** The list is scrolled in whole rows when it is longer than the screen. */
      scroll: 0,
      /** How many rows fit, set by `applyLayout` from the actual type size. */
      visibleRows: 12,
    }
  },
  computed: {
    layout() {
      const rowHeight = Math.round(TOKENS.type.body * 2.1)
      const gap = Math.round(TOKENS.type.caption * 0.6)
      const top = Math.round(TOKENS.spacing.shellPad + TOKENS.type.title * 1.4 + TOKENS.type.body * 3)
      const footY = 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 2)
      return {
        top,
        rowHeight,
        rowGap: rowHeight + gap,
        fieldX: Math.round(TOKENS.spacing.fieldX),
        valueX: 620,
        rowLabelY: Math.round(rowHeight * 0.3),
        footY,
        visibleRows: Math.max(1, Math.floor((footY - top) / (rowHeight + gap))),
      }
    },
  },
  hooks: {
    ready() {
      this.$focus()
      this.applyLayout()
      this.reload()
      // The drive list is re-read on open, not cached from app start: a USB stick
      // plugged in while the app was on Home must be visible here immediately.
      void this.refreshDrives()
    },
  },
  /**
   * The remote keys, as the engine names them.
   *
   * Declaring them is what makes them reachable: the engine only delivers a key to a
   * component whose `input` names it, and this block is the whole wiring.
   */
  input: {
    up() {
      this.up()
    },
    down() {
      this.down()
    },
    enter() {
      void this.enter()
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
      this.visibleRows = layout.visibleRows
    },
    /** Reads every host fact again and rebuilds the list. */
    reload() {
      const capabilities = host()
      const info = device()
      const registration = keys()
      const report = library().loadReport()
      const store = library().state()
      const itemCount = store.items.length
      const unavailable = store.items.filter((item) => !item.available).length
      const nowMs = Date.now()
      const lastScan: ScanReport = scanReport()
      const prefs: DeviceSettings = settings()

      this.subtitle = hostSummary(capabilities)
      const entries: Entry[] = [
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
          // Separate from the Tizen version on purpose: the TV's product version is a
          // different number (3.7 on this set), and showing it as the platform version
          // would be exactly the kind of confident-but-wrong receipt this app avoids.
          key: 'product',
          label: 'Product version',
          value: info.productVersion ?? 'not reported by this host',
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
        { key: 'storage', label: 'Where your data is kept', value: report.location },
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
        { key: 'writefail', label: 'Last write', value: library().writeFailure()?.reason ?? 'accepted' },
        { key: 'drives', label: 'Drives the TV reports', value: storageSummary() },
      ]

      // The drives themselves, one line each: a device answer that contradicts the
      // documentation is visible here rather than inferred from an empty library.
      const roots = mediaRoots()
      for (const root of roots.slice(0, MAX_LISTED)) {
        entries.push({
          key: `root-${root.label}`,
          label: `Drive ${root.label}`,
          value: `${root.type.toLowerCase()} · ${root.state.toLowerCase()} · ${
            root.scannable ? 'this app scans it' : 'not scanned by this app'
          }`,
        })
      }
      if (roots.length > MAX_LISTED) {
        entries.push({
          key: 'root-more',
          label: 'More drives',
          value: `${String(roots.length - MAX_LISTED)} not listed`,
        })
      }

      for (const source of store.sources.slice(0, MAX_LISTED)) {
        const count = store.items.filter((item) => item.sourceId === source.id).length
        entries.push({
          key: `source-${source.id}`,
          label: `Library ${source.label}`,
          value: `${source.state} · ${count} ${count === 1 ? 'file' : 'files'} · scanned ${sinceMs(
            source.lastScanMs,
            nowMs,
          )}`,
        })
      }
      if (store.sources.length > MAX_LISTED) {
        entries.push({
          key: 'source-more',
          label: 'More library roots',
          value: `${String(store.sources.length - MAX_LISTED)} not listed`,
        })
      }

      entries.push({
        key: 'sources',
        label: 'Catalogued',
        value: `${itemCount} ${itemCount === 1 ? 'file' : 'files'}${
          unavailable > 0 ? `, ${String(unavailable)} not readable now` : ''
        }`,
      })
      entries.push({
        key: 'lastscan',
        label: 'Last scan',
        value: `${describeScan(lastScan)}${lastScan.finishedMs > 0 ? ` (${sinceMs(lastScan.finishedMs, nowMs)})` : ''}`,
      })
      if (lastScan.issues.length > 0) {
        const first = lastScan.issues[0]
        entries.push({
          key: 'scanissues',
          label: 'Entries the scan refused',
          value: first === undefined ? '' : `${String(lastScan.issues.length)} · first: ${first.code} (${first.relativePath === '' ? 'whole drive' : first.relativePath})`,
        })
      }

      entries.push({
        key: 'subtitles',
        label: 'Subtitles',
        value: describeSubtitles(prefs),
        action: 'subtitles',
      })
      entries.push({
        key: 'audio',
        label: 'Audio language',
        value: describeAudio(prefs),
        action: 'audio',
      })
      entries.push({
        key: 'forget',
        label: 'Forget learned languages',
        value: 'press Enter',
        action: 'forget',
      })
      entries.push({
        key: 'scan',
        label: 'Scan for media',
        value: this.busy
          ? 'scanning — press Enter to stop'
          : scanInProgress()
            ? 'a scan is already running'
            : 'press Enter',
        action: 'scan',
      })
      entries.push({ key: 'provenance', label: 'Design tokens', value: TOKENS.provenance })

      const layout = this.layout
      this.visibleRows = layout.visibleRows
      // The list can shrink between reloads (a drive removed, a scan problem cleared),
      // so the window is clamped rather than left scrolled past the end.
      this.scroll = Math.min(Math.max(0, entries.length - this.visibleRows), Math.max(0, this.scroll))
      this.rows = entries.map((entry, index) => {
        const focused = index === this.focusIndex
        return {
          key: entry.key,
          label: entry.label,
          value: entry.value,
          // Rows are placed from the top of the visible window, so a list longer than
          // the screen scrolls rather than running off the bottom of the television.
          y: layout.top + (index - this.scroll) * layout.rowGap,
          fill: focused ? hex(TOKENS.palette.focusSurface) : hex(TOKENS.palette.surface1),
          labelColor: focused ? hex(TOKENS.palette.focusForeground) : hex(TOKENS.palette.foreground),
          valueColor: focused ? hex(TOKENS.palette.focusForeground) : hex(TOKENS.palette.inkMuted),
          focused,
          actionable: entry.action !== undefined,
          action: entry.action ?? null,
        }
      })
      this.focusIndex = clampIndex(this.focusIndex, this.rows.length)
      this.footer = onTv()
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
          y: layout.top + (index - this.scroll) * layout.rowGap,
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
      // Keep the cursor inside the visible window rather than letting it walk off the
      // screen: a highlighted row nobody can see is a cursor that has been lost.
      const first = this.scroll
      const last = this.scroll + this.visibleRows - 1
      if (next < first) {
        this.scroll = next
      } else if (next > last) {
        this.scroll = next - this.visibleRows + 1
      }
      this.markFocus()
    },
    /** Enter: whichever action the highlighted row names. */
    async enter() {
      const row = this.rows[this.focusIndex]
      if (row === undefined || row.action === null) {
        return
      }
      switch (row.action) {
        case 'scan':
          if (this.busy) {
            // Enter during a scan stops it, because a walk that cannot be stopped is a
            // walk that holds the drive until the app is closed.
            cancelScan()
            this.footer = 'Stopping after the folder it is in…'
            return
          }
          await this.runScan()
          return
        case 'subtitles': {
          const flipped = settings().subtitles === 'off' ? 'auto' : 'off'
          const next: DeviceSettings = { ...settings(), subtitles: flipped }
          saveSettings(next)
          this.reload()
          this.footer = `Subtitles: ${describeSubtitles(settings())}`
          return
        }
        case 'audio': {
          // The only honest way to "unset" a language this app learned is to leave the
          // choice to each file again, which is what an empty preference means.
          saveSettings({ ...settings(), audioLanguage: '' })
          this.reload()
          this.footer = 'Audio is back to whatever each file defaults to.'
          return
        }
        case 'forget': {
          saveSettings(forgetChoices())
          this.reload()
          this.footer = 'Both preferences are back to this television’s defaults.'
          return
        }
      }
    },
    /** Asks the TV for its drives and walks them. */
    async runScan() {
      this.busy = true
      this.reload()
      // The drive list is refreshed by the scan itself, so the screen does not race it.
      const report = await scanDrives()
      this.busy = false
      this.reload()
      this.footer = describeScan(report)
    },
    /** Re-reads the drive list without scanning. Used when the screen opens. */
    async refreshDrives() {
      await refreshMediaRoots()
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
