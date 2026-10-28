/**
 * Browse: one destination's library, as a scrollable grid.
 *
 * The same component draws Movies and TV Shows, because the only thing that differs is
 * which rows it selects; a second screen would be a second copy of this focus code and
 * a second place for the two lists to disagree.
 *
 * Paging is gone, on purpose. A page was a whole number of rows and the remote's
 * rewind/forward keys turned it — which meant the library could only be moved through
 * with two keys most remotes do not label, and a thousand files needed a hundred
 * presses the viewer could not predict. The grid is now a window over the whole list:
 * Down is always the next row, the band slides under the cursor, and only ten cards
 * exist at a time. Rewind and forward still turn a page for anyone who knows them, but
 * nothing needs them.
 *
 * The rail sits beside the grid and is a real focus zone: Left from the first column
 * goes to it, Right comes back to the card the viewer left.
 */

import Blits from '@lightningjs/blits'

import { COLUMNS, emptyState, itemsFor, ROWS, type KindFilter } from '../core/browse.ts'
import {
  gridWindow,
  moveNav,
  RAIL_ENTRIES,
  railIndexFor,
  restoreNav,
  type NavPosition,
  type NavZone,
} from '../core/nav.ts'
import { APP_WIDTH, buildCards, gridFor } from '../design/cards.ts'
import { railRows, RAIL_WIDTH, type RailRow } from '../design/rail.ts'
import Rail from '../components/Rail.ts'
import { hex, TOKENS } from '../design/tokens.ts'
import { items, nav, onTv, scanInProgress, sources } from '../app/runtime.ts'

/** The grid starts just right of the rail. */
const CONTENT_LEFT = RAIL_WIDTH + Math.round(TOKENS.spacing.shellPad * 0.6)

/** The route this screen remembers its cursor under, per destination. */
function routeFor(kind: KindFilter): string {
  switch (kind) {
    case 'all':
      return '/browse'
    case 'movies':
      return '/movies'
    case 'series':
      return '/series'
  }
}

/** A destination's heading, and what its empty screen should say. */
function headingFor(kind: KindFilter): string {
  switch (kind) {
    case 'all':
      return 'All media'
    case 'movies':
      return 'Movies'
    case 'series':
      return 'TV Shows'
  }
}

/** The prop, narrowed: an unknown value shows everything rather than nothing. */
function narrowKind(value: unknown): KindFilter {
  return value === 'movies' || value === 'series' ? value : 'all'
}

export default Blits.Component('Browse', {
  components: { Rail },
  props: {
    /** Which rows this destination shows. Set by the route, not by the viewer. */
    kind: { default: 'all' },
  },
  template: `
    <Element w="1920" h="1080" :color="$background">
      <Rail :rows="$railRows" />

      <Text
        :x="$contentLeft"
        :y="$pad"
        :content="$heading"
        :size="$headingSize"
        :color="$foreground"
      />
      <Text
        :x="$contentLeft"
        :y="$subY"
        :content="$subheading"
        :size="$bodySize"
        :color="$muted"
        maxlines="1"
        textoverflow="ellipsis"
        :maxwidth="$noteWidth"
      />

      <Element
        :for="card in $cards"
        key="$card.id"
        :x="$card.x"
        :y="$card.y"
        :w="$card.width"
        :h="$card.height"
        :color="$card.fill"
        :scale="$card.scale"
        :rounded="$radius"
      >
        <Element
          x="0"
          y="0"
          :w="$card.width"
          :h="$artHeight"
          :color="$surface2"
          :rounded="$radius"
        />
        <Text
          :x="$fieldX"
          :y="$titleY"
          :content="$card.title"
          :size="$titleSize"
          :color="$card.titleColor"
          maxlines="1"
          textoverflow="ellipsis"
          :maxwidth="$cardTextWidth"
        />
        <Text
          :x="$fieldX"
          :y="$subtitleY"
          :content="$card.subtitle"
          :size="$captionSize"
          :color="$card.subtitleColor"
          maxlines="1"
          textoverflow="ellipsis"
          :maxwidth="$cardTextWidth"
        />
        <Element
          x="0"
          :y="$progressY"
          :w="$card.progressWidth"
          :h="$barHeight"
          :color="$card.progressColor"
          :show="$card.progressVisible"
        />
      </Element>

      <Element :show="$emptyTitle !== ''">
        <Text
          :x="$contentLeft"
          :y="$emptyY"
          :content="$emptyTitle"
          :size="$titleSize"
          :color="$foreground"
        />
        <Text
          :x="$contentLeft"
          :y="$emptyDetailY"
          :content="$emptyDetail"
          :size="$bodySize"
          :color="$muted"
          maxlines="3"
          wordwrap="true"
          :maxwidth="$emptyWidth"
        />
      </Element>

      <Text :x="$contentLeft" :y="$footY" :content="$footer" :size="$captionSize" :color="$disabled" />
    </Element>
  `,
  state() {
    return {
      railRows: [] as RailRow[],
      background: hex(TOKENS.palette.background),
      foreground: hex(TOKENS.palette.foreground),
      muted: hex(TOKENS.palette.inkMuted),
      disabled: hex(TOKENS.palette.disabledForeground),
      surface2: hex(TOKENS.palette.surface2),
      pad: Math.round(TOKENS.spacing.shellPad),
      contentLeft: CONTENT_LEFT,
      noteWidth: APP_WIDTH - CONTENT_LEFT - Math.round(TOKENS.spacing.shellPad),
      radius: Math.round(TOKENS.radius.surface),
      headingSize: Math.round(TOKENS.type.title),
      titleSize: Math.round(TOKENS.type.bodyLg),
      bodySize: Math.round(TOKENS.type.body),
      captionSize: Math.round(TOKENS.type.caption),
      subY: Math.round(TOKENS.spacing.shellPad + TOKENS.type.title * 1.4),
      // The grid's numbers, copied out of `computed.grid` by `applyLayout` before the
      // first paint. A template resolves `$name` against state only, so a value that
      // lives on a computed object is `undefined` here and its node is never drawn.
      artHeight: 0,
      fieldX: 0,
      titleY: 0,
      subtitleY: 0,
      progressY: 0,
      barHeight: 6,
      cardTextWidth: 0,
      emptyY: 0,
      emptyDetailY: 0,
      emptyWidth: 0,
      footY: 0,
      heading: 'All media',
      subheading: '',
      footer: '',
      emptyTitle: '',
      emptyDetail: '',
      cards: [] as ReturnType<typeof buildCards>,
      /** The absolute index of the first drawn card. */
      listFirst: 0,
      count: 0,
      zone: 'content' as NavZone,
      railIndex: 0,
      contentIndex: 0,
    }
  },
  computed: {
    /** The destination this screen was opened for. */
    destination(): KindFilter {
      return narrowKind(this.kind)
    },
    /** The route this screen belongs to, which is also its focus-memory key. */
    route(): string {
      return routeFor(this.destination)
    },
    grid() {
      const top = Math.round(
        TOKENS.spacing.shellPad + TOKENS.type.title * 1.4 + TOKENS.type.body * 2.6,
      )
      return gridFor(COLUMNS, ROWS, TOKENS, APP_WIDTH, {
        top,
        bottom: 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 2.4),
        left: CONTENT_LEFT,
      })
    },
  },
  hooks: {
    ready() {
      this.$focus()
      this.applyLayout()
      this.reload()
    },
    focus() {
      this.reload()
    },
  },
  /** The remote keys. See the note in `Home.ts` for why this block has to exist. */
  input: {
    left() {
      this.press('left')
    },
    right() {
      this.press('right')
    },
    up() {
      this.press('up')
    },
    down() {
      this.press('down')
    },
    enter() {
      this.enter()
    },
    rewind() {
      this.jump(-1)
    },
    fastForward() {
      this.jump(1)
    },
  },
  methods: {
    /** Copies the derived geometry into state, where the template can see it. */
    applyLayout(): void {
      const grid = this.grid
      this.artHeight = grid.artHeight
      this.fieldX = Math.round(TOKENS.spacing.fieldX)
      this.titleY = grid.titleY
      this.subtitleY = grid.subtitleY
      this.progressY = grid.artHeight - 6
      this.barHeight = 6
      this.cardTextWidth = grid.cardWidth - this.fieldX * 2
      this.emptyY = Math.round(TOKENS.spacing.shellPad + TOKENS.type.title * 2.6)
      this.emptyDetailY = Math.round(TOKENS.spacing.shellPad + TOKENS.type.title * 3.6)
      this.emptyWidth = APP_WIDTH - CONTENT_LEFT - Math.round(TOKENS.spacing.shellPad)
      this.footY = 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 2.4)
    },
    /** The rows this destination shows, in display order. */
    visible(): ReturnType<typeof itemsFor> {
      return itemsFor(items(), this.destination)
    },
    /** The cursor's bounds, which depend on this destination's row count. */
    bounds(): { railCount: number; contentCount: number; columns: number } {
      return {
        railCount: RAIL_ENTRIES.length,
        contentCount: this.visible().length,
        columns: COLUMNS,
      }
    },
    /** Reads the store, restores the cursor, and rebuilds the grid. */
    reload(): void {
      this.heading = headingFor(this.destination)
      const position = restoreNav(nav(), this.route, this.bounds(), 'content')
      this.zone = position.zone
      this.railIndex = position.rail
      this.contentIndex = position.content
      this.repaint()

      const all = items()
      const granted = sources()
      const situation = emptyState({
        sourceCount: granted.length,
        catalogCount: all.length,
        scanning: scanInProgress(),
        unavailableSources: granted.filter((source) => source.state !== 'available').length,
        filterText: '',
      })
      const own = this.visible().length
      if (situation !== null) {
        this.emptyTitle = situation.title
        this.emptyDetail = situation.detail
      } else if (own === 0) {
        // The library has files, but none of this destination's kind. Saying which kind
        // is missing, and where the others are, is the difference between an empty
        // screen and a dead end.
        this.emptyTitle =
          this.destination === 'series' ? 'No TV shows yet.' : 'No films yet.'
        this.emptyDetail =
          this.destination === 'series'
            ? 'Episodes are recognised from names like Show.S01E01.mkv. Everything else is under Movies.'
            : 'Everything recognised as an episode is under TV Shows. Check Media Sources if a drive is missing.'
      } else {
        this.emptyTitle = ''
        this.emptyDetail = ''
      }

      this.footer = onTv()
        ? 'OK opens · ◄ ► ▲ ▼ move · Left on the first card opens the menu'
        : 'Browser preview — nothing here proves TV playback'
    },
    /** Rebuilds the visible window. Only ten cards exist, whatever the library holds. */
    repaint(): void {
      const rows = this.visible()
      const total = rows.length
      this.count = total
      this.contentIndex = total === 0 ? -1 : Math.min(this.contentIndex, total - 1)

      const window = gridWindow(total, COLUMNS, ROWS, this.contentIndex)
      this.listFirst = window.first
      this.cards = buildCards(
        rows.slice(window.first, window.first + window.count),
        // The rail's cursor is the only highlight while it holds the cursor; see the
        // same note in `Home.ts`.
        this.zone === 'content' ? this.contentIndex : -1,
        this.grid,
        TOKENS,
        { first: window.first, columns: COLUMNS },
      )
      this.railRows = railRows(railIndexFor(this.route), this.railIndex, this.zone === 'rail')
      this.subheading =
        total === 0
          ? ''
          : `${total} ${total === 1 ? 'file' : 'files'} · ${Math.floor(
              Math.max(0, this.contentIndex) / COLUMNS,
            ) + 1} of ${window.totalRows} rows`
      nav().set(
        this.route,
        { zone: this.zone, rail: this.railIndex, content: this.contentIndex },
        this.bounds(),
      )
    },
    /** One D-pad press, by the rules `core/nav.ts` tests. */
    press(direction: 'left' | 'right' | 'up' | 'down'): void {
      const current: NavPosition = {
        zone: this.zone,
        rail: this.railIndex,
        content: this.contentIndex,
      }
      const next = moveNav(current, direction, this.bounds())
      if (
        next.zone === this.zone &&
        next.rail === this.railIndex &&
        next.content === this.contentIndex
      ) {
        return
      }
      this.zone = next.zone
      this.railIndex = next.rail
      this.contentIndex = next.content
      this.repaint()
    },
    /**
     * Turns a page with the rewind and forward keys.
     *
     * Nothing needs these: Down already walks the whole list. They are kept because a
     * viewer who knows them should not lose them, and because a page of rows is a much
     * faster way down a long drive.
     */
    jump(direction: number): void {
      if (this.zone !== 'content' || this.count === 0) {
        return
      }
      const step = COLUMNS * ROWS
      const next = Math.min(this.count - 1, Math.max(0, this.contentIndex + direction * step))
      if (next === this.contentIndex) {
        return
      }
      this.contentIndex = next
      this.repaint()
    },
    /** OK on the rail opens a destination; OK on a card opens the file. */
    enter(): void {
      if (this.zone === 'rail') {
        const entry = RAIL_ENTRIES[this.railIndex]
        if (entry === undefined) {
          return
        }
        if (entry.route !== this.route) {
          this.$router.to(entry.route)
          return
        }
        // OK on the destination already open is not a dead key: it hands the cursor to
        // the grid, which is the only thing that row could usefully do from here.
        if (this.count > 0) {
          this.zone = 'content'
          this.contentIndex = Math.max(0, this.contentIndex)
          this.repaint()
        }
        return
      }
      const card = this.cards.find((candidate) => candidate.focused)
      if (card === undefined) {
        return
      }
      this.$router.to('/detail', { id: card.id })
    },
  },
})
