/**
 * Home: what the owner was watching.
 *
 * The screen is deliberately one focus zone, because that is the whole reason
 * `core/focus.ts` exists: a single flat index into a grid, moved by the tested rules
 * and remembered per screen so Back returns the cursor where it was. Nothing here
 * re-implements an arrow key.
 *
 * The library is read once per visit (`focus` hook) rather than watched, because the
 * store is a synchronous document and the screens are cheap to rebuild. What is *not*
 * deferred is the empty state: an owner with no media and an owner whose drive is
 * unplugged need different sentences, and `core/browse.ts` already decides which one
 * applies.
 */

import Blits from '@lightningjs/blits'

import { continueWatching, emptyState } from '../core/browse.ts'
import { moveFocus, restoreFocus, type Direction } from '../core/focus.ts'
import { APP_WIDTH, buildCards, gridFor } from '../design/cards.ts'
import { hex, TOKENS } from '../design/tokens.ts'
import {
  focus as focusMemory,
  items,
  mediaRoots,
  onTv,
  scanInProgress,
  sources,
} from '../app/runtime.ts'

/** How many resume entries Home shows. One full screen, no paging. */
const HOME_COLUMNS = 4

/** Two rows of four: the eight most recent, which is what one screen holds. */
const HOME_ROWS = 2

/** The route this screen remembers its cursor under. */
const ROUTE = '/'

export default Blits.Component('Home', {
  template: `
    <Element w="1920" h="1080" :color="$background">
      <Text
        :x="$pad"
        :y="$pad"
        :content="$heading"
        :size="$headingSize"
        :color="$foreground"
      />
      <Text
        :x="$pad"
        :y="$subY"
        :content="$subheading"
        :size="$bodySize"
        :color="$muted"
        maxlines="2"
        wordwrap="true"
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
          :maxwidth="$card.width"
        />
        <Text
          :x="$fieldX"
          :y="$subtitleY"
          :content="$card.subtitle"
          :size="$captionSize"
          :color="$card.subtitleColor"
          maxlines="1"
          textoverflow="ellipsis"
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
        <Text :x="$pad" :y="$emptyY" :content="$emptyTitle" :size="$titleSize" :color="$foreground" />
        <Text
          :x="$pad"
          :y="$emptyDetailY"
          :content="$emptyDetail"
          :size="$bodySize"
          :color="$muted"
          maxlines="3"
          wordwrap="true"
          maxwidth="1180"
        />
      </Element>

      <Text :x="$pad" :y="$footY" :content="$footer" :size="$captionSize" :color="$disabled" />
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
      // The grid's own numbers, copied out of `computed.grid` by `applyLayout` before the
      // first paint. They are state rather than computed properties because a template
      // only resolves `$name` against the component's state: a value that exists solely
      // on a computed *object* is `undefined` here, which places the node nowhere and
      // draws nothing. This is why the first installed build showed one line and no grid.
      artHeight: 0,
      fieldX: 0,
      titleY: 0,
      subtitleY: 0,
      progressY: 0,
      barHeight: 6,
      emptyY: 0,
      emptyDetailY: 0,
      footY: 0,
      heading: 'Continue watching',
      subheading: '',
      footer: '',
      emptyTitle: '',
      emptyDetail: '',
      cards: [] as ReturnType<typeof buildCards>,
      focusIndex: 0,
      count: 0,
    }
  },
  computed: {
    /**
     * The grid, sized to the band this screen leaves for it.
     *
     * `gridFor` shrinks the cards until both rows fit between this screen's header and
     * its footer, so nothing is drawn past the bottom of the television.
     */
    grid() {
      const geometry = gridFor(HOME_COLUMNS, HOME_ROWS, TOKENS, APP_WIDTH, {
        top: Math.round(
          TOKENS.spacing.shellPad + TOKENS.type.title * 1.4 + TOKENS.type.body * 4,
        ),
        bottom: 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 2),
      })
      return {
        ...geometry,
        progressY: geometry.artHeight - 6,
        barHeight: 6,
        fieldX: Math.round(TOKENS.spacing.fieldX),
        emptyY: Math.round(TOKENS.spacing.shellPad + TOKENS.type.title * 2.6),
        emptyDetailY: Math.round(TOKENS.spacing.shellPad + TOKENS.type.title * 3.6),
        footY: 1080 - Math.round(TOKENS.spacing.shellPad + TOKENS.type.body * 2),
      }
    },
  },
  hooks: {
    ready() {
      this.applyLayout()
      this.reload()
    },
    focus() {
      this.reload()
    },
  },
  /**
   * The remote keys, as the engine names them.
   *
   * This block is not decoration: an input handler is only reachable when it is
   * declared here, and the screen's own `methods` are what it calls. Without it the
   * arrow keys arrive and go nowhere.
   */
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
  },
  methods: {
    /**
     * Copies the derived geometry into state, where the template can see it.
     *
     * `grid` stays a computed property, because `repaint` wants the whole object and the
     * numbers are a pure function of the token set. The template, however, reads state,
     * so each key it names has to exist on the component itself.
     */
    applyLayout(): void {
      const grid = this.grid
      this.artHeight = grid.artHeight
      this.fieldX = grid.fieldX
      this.titleY = grid.titleY
      this.subtitleY = grid.subtitleY
      this.progressY = grid.progressY
      this.barHeight = grid.barHeight
      this.emptyY = grid.emptyY
      this.emptyDetailY = grid.emptyDetailY
      this.footY = grid.footY
    },
    /** Reads the store and rebuilds what this screen shows. */
    reload() {
      const entries = continueWatching(items(), HOME_COLUMNS * HOME_ROWS)
      this.count = entries.length
      // Back should return the cursor where it was, not to the first card — and a
      // rescan that removed files must not leave it pointing past the end.
      this.focusIndex = this.count > 0 ? restoreFocus(focusMemory(), ROUTE, this.count) : 0
      this.repaint()

      const granted = sources()
      const situation = emptyState({
        sourceCount: granted.length,
        catalogCount: items().length,
        scanning: scanInProgress(),
        unavailableSources: granted.filter((source) => source.state !== 'available').length,
        filterText: '',
      })
      if (situation === null) {
        this.emptyTitle = ''
        this.emptyDetail = ''
      } else {
        this.emptyTitle = situation.title
        this.emptyDetail = situation.detail
      }

      const total = items().length
      this.subheading =
        total > 0
          ? `${total} ${total === 1 ? 'file' : 'files'} in the library.`
          : 'Nothing catalogued yet.'
      this.footer = this.footerFor()
      // Remember where the cursor is, so returning to Home restores it.
      focusMemory().set(ROUTE, this.focusIndex, Math.max(1, this.count))
    },
    footerFor(): string {
      const drives = mediaRoots().length
      const driveLine =
        drives === 0 ? 'no drive found' : `${drives} ${drives === 1 ? 'drive' : 'drives'} found`
      return onTv()
        ? `Enter plays · Info opens Settings · ${driveLine}`
        : `Browser preview — nothing here proves TV playback · ${driveLine}`
    },
    /** Moves the cursor by one press, by the rules `core/focus.ts` tests. */
    move(direction: Direction) {
      const next = moveFocus(this.focusIndex, this.count, HOME_COLUMNS, direction)
      if (next === this.focusIndex) {
        return
      }
      this.focusIndex = next
      this.repaint()
      focusMemory().set(ROUTE, this.focusIndex, Math.max(1, this.count))
    },
    /** Rebuilds the cards for the current cursor, which is what the template shows. */
    repaint() {
      const entries = continueWatching(items(), HOME_COLUMNS * HOME_ROWS)
      this.cards = buildCards(
        entries.map((entry) => entry.item),
        this.focusIndex,
        this.grid,
        TOKENS,
      )
    },
    /** Enter on the focused card. */
    enter() {
      const card = this.cards[this.focusIndex]
      if (card === undefined) {
        return
      }
      this.$router.to('/detail', { id: card.id })
    },
    /** The remote's Info key, which is where Settings lives. */
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
  },
})
