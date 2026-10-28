/**
 * Browse: the whole library, paged.
 *
 * Paging rather than scrolling is a deliberate TV choice, and it is the same one the
 * desktop makes: a page is a whole number of rows (`core/browse.ts`), so a page never
 * ends half-way through a row and the next press always lands on a real card.
 *
 * The page keys are the remote's rewind and fast-forward. That mapping is not
 * decoration — a TV has no scrollbar and no wheel, so paging needs a key, and those
 * two are the only ones on the remote whose meaning ("earlier" and "later") already
 * matches what they do here.
 *
 * The kind and text filters are implemented and tested in `core/browse.ts` but have no
 * UI on this screen yet; the header therefore says what is being shown, so the owner is
 * never looking at a filtered list that does not say it is filtered. That gap is
 * recorded in the Tizen milestone notes rather than half-built.
 */

import Blits from '@lightningjs/blits'

import { COLUMNS, emptyState, pageCount, pageItems, ROWS } from '../core/browse.ts'
import { clampIndex, moveFocus, restoreFocus, type Direction } from '../core/focus.ts'
import { buildCards, gridFor } from '../design/cards.ts'
import { hex, TOKENS } from '../design/tokens.ts'
import type { LibraryItem } from '../core/contract.ts'
import {
  focus as focusMemory,
  items,
  onTv,
  scanInProgress,
  sources,
} from '../app/runtime.ts'

const ROUTE = '/browse'

export default Blits.Component('Browse', {
  template: `
    <Element w="1920" h="1080" color="$background">
      <Text x="$pad" y="$pad" content="$heading" size="$headingSize" color="$foreground" />
      <Text x="$pad" y="$subY" content="$subheading" size="$bodySize" color="$muted" />

      <Element
        :for="card in $cards"
        key="$card.id"
        x="$card.x"
        y="$card.y"
        w="$card.width"
        h="$card.height"
        :color="$card.fill"
        :scale="$card.scale"
        rounded="$radius"
      >
        <Element
          x="0"
          y="0"
          w="$card.width"
          h="$artHeight"
          color="$surface2"
          rounded="$radius"
        />
        <Text
          x="$fieldX"
          y="$titleY"
          content="$card.title"
          size="$titleSize"
          color="$card.titleColor"
          maxlines="1"
          textoverflow="ellipsis"
        />
        <Text
          x="$fieldX"
          y="$subtitleY"
          content="$card.subtitle"
          size="$captionSize"
          color="$card.subtitleColor"
          maxlines="1"
          textoverflow="ellipsis"
        />
        <Element
          x="0"
          :y="$progressY"
          :w="$card.progressWidth"
          h="$barHeight"
          :color="$card.progressColor"
          :show="$card.progressVisible"
        />
      </Element>

      <Element :show="$emptyTitle !== ''">
        <Text x="$pad" y="$emptyY" content="$emptyTitle" size="$titleSize" color="$foreground" />
        <Text
          x="$pad"
          y="$emptyDetailY"
          content="$emptyDetail"
          size="$bodySize"
          color="$muted"
          maxlines="3"
          wordwrap="true"
          maxwidth="1180"
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
      pad: Math.round(TOKENS.spacing.shellPad),
      radius: Math.round(TOKENS.radius.surface),
      headingSize: Math.round(TOKENS.type.title),
      titleSize: Math.round(TOKENS.type.bodyLg),
      bodySize: Math.round(TOKENS.type.body),
      captionSize: Math.round(TOKENS.type.caption),
      subY: Math.round(TOKENS.spacing.shellPad + TOKENS.type.title * 1.4),
      heading: 'Library',
      subheading: '',
      footer: '',
      emptyTitle: '',
      emptyDetail: '',
      cards: [] as ReturnType<typeof buildCards>,
      focusIndex: 0,
      page: 0,
      pages: 1,
      count: 0,
    }
  },
  computed: {
    grid() {
      return gridFor(COLUMNS, ROWS)
    },
    metrics() {
      const geometry = this.grid
      return {
        artHeight: geometry.artHeight,
        titleY: geometry.artHeight + Math.round(TOKENS.type.caption),
        subtitleY: geometry.artHeight + Math.round(TOKENS.type.caption + TOKENS.type.bodyLg * 1.5),
        progressY: geometry.artHeight - 6,
        barHeight: 6,
        fieldX: Math.round(TOKENS.spacing.fieldX),
        gridOffsetY: Math.round(TOKENS.spacing.shellPad + TOKENS.type.title * 1.4 + TOKENS.type.body * 3),
        emptyY: Math.round(TOKENS.spacing.shellPad + TOKENS.type.title * 2.6),
        emptyDetailY: Math.round(TOKENS.spacing.shellPad + TOKENS.type.title * 3.6),
        footY: 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 2),
      }
    },
  },
  hooks: {
    ready() {
      this.$focus()
      this.reload()
    },
    focus() {
      this.reload()
    },
  },
  /** The remote keys. See the note in `Home.ts` for why this block has to exist. */
  input: {
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
    enter() {
      this.enter()
    },
    info() {
      this.info()
    },
    rewind() {
      this.rewind()
    },
    fastForward() {
      this.fastForward()
    },
  },
  methods: {
    /** Reads the store, clamps the page, and rebuilds the visible page. */
    reload() {
      const all = items()
      this.pages = pageCount(all.length)
      this.page = Math.min(this.page, this.pages - 1)
      const visible = pageItems(all, this.page)
      this.count = visible.length
      this.focusIndex =
        this.count > 0 ? restoreFocus(focusMemory(), ROUTE, this.count) : 0
      this.cards = this.build(visible)

      const granted = sources()
      const situation = emptyState({
        sourceCount: granted.length,
        catalogCount: all.length,
        scanning: scanInProgress(),
        unavailableSources: granted.filter((source) => source.state !== 'available').length,
        filterText: '',
      })
      this.emptyTitle = situation?.title ?? ''
      this.emptyDetail = situation?.detail ?? ''
      this.subheading = this.subheadingFor(all.length)
      this.footer = this.footerFor()
      focusMemory().set(ROUTE, this.focusIndex, Math.max(1, this.count))
    },
    subheadingFor(total: number): string {
      if (total === 0) {
        return 'Nothing catalogued yet.'
      }
      return `All media · page ${this.page + 1} of ${this.pages} · ${total} ${total === 1 ? 'file' : 'files'}`
    },
    footerFor(): string {
      return onTv()
        ? 'Enter plays · ►| next page · |◄ previous page · Back returns'
        : 'Browser preview — nothing here proves TV playback'
    },
    /** Places the page's cards, offset below this screen's header. */
    build(visible: readonly LibraryItem[]): ReturnType<typeof buildCards> {
      const geometry = this.grid
      const offset = this.metrics.gridOffsetY - geometry.originY
      return buildCards(visible, this.focusIndex, geometry, TOKENS).map((card) => ({
        ...card,
        y: card.y + offset,
      }))
    },
    /** Moves the cursor by one press, by the rules `core/focus.ts` tests. */
    move(direction: Direction) {
      const next = moveFocus(this.focusIndex, this.count, COLUMNS, direction)
      if (next === this.focusIndex) {
        return
      }
      this.focusIndex = next
      this.repaint()
      focusMemory().set(ROUTE, this.focusIndex, Math.max(1, this.count))
    },
    repaint() {
      this.cards = this.build(pageItems(items(), this.page))
    },
    /** Turns the page, keeping the cursor inside it. */
    turn(delta: number) {
      const next = Math.min(this.pages - 1, Math.max(0, this.page + delta))
      if (next === this.page) {
        return
      }
      this.page = next
      const visible = pageItems(items(), this.page)
      this.count = visible.length
      // The cursor is a flat index into the *page*, so it is clamped rather than
      // carried: the card at index 7 of the old page is a different file on the new
      // one, and moving the cursor to it would be a jump the user did not ask for.
      this.focusIndex = clampIndex(0, this.count)
      this.cards = this.build(visible)
      this.subheading = this.subheadingFor(items().length)
      focusMemory().set(ROUTE, this.focusIndex, Math.max(1, this.count))
    },
    enter() {
      const card = this.cards[this.focusIndex]
      if (card === undefined) {
        return
      }
      this.$router.to('/detail', { id: card.id })
    },
    info() {
      this.$router.to('/settings')
    },
    left() {
      this.move('left')
    },
    right() {
      this.move('right')
    },
    up() {
      this.move('up')
    },
    down() {
      this.move('down')
    },
    rewind() {
      this.turn(-1)
    },
    fastForward() {
      this.turn(1)
    },
  },
})
