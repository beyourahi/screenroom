/**
 * The navigation rail, as a component.
 *
 * It is a child component rather than a copied block in six templates because the rail
 * *is* one menu: five screens embedding their own copy would drift, and the drift would
 * only show on a television. Each screen owns the cursor state and passes the drawn rows
 * in; this component owns nothing but its own measurements.
 *
 * A previous attempt embedded a shared markup *string* into each screen's template with
 * `${...}`. That does not survive Blits' template precompiler, which matches a template
 * literal and replaces it: the interpolation was left as text, and the entire menu drew
 * nothing while the rest of the screen looked correct. The lesson is recorded here
 * because the symptom — a missing menu with no error — is exactly what this component
 * exists to prevent.
 */

import Blits from '@lightningjs/blits'

import type { RailRow } from '../design/rail.ts'
import { RAIL_WIDTH, railLayout } from '../design/rail.ts'
import { hex, TOKENS } from '../design/tokens.ts'

export default Blits.Component('Rail', {
  props: {
    /**
     * The rows to draw, already decided.
     *
     * The screen decides which destination is active and which row the remote is on,
     * because only the screen knows its own route and zone. This component only places
     * them, so a change of cursor is a new array here and nothing else.
     */
    rows: { default: [] as RailRow[] },
  },
  template: `
    <Element x="0" y="0" :w="$width" h="1080" :color="$background">
      <Element :x="$dividerX" y="0" w="2" h="1080" :color="$border" />
      <Text
        :x="$pad"
        :y="$pad"
        content="Screenroom"
        :size="$titleSize"
        :color="$headingColor"
      />
      <Element
        :for="row in $rows"
        key="$row.key"
        :x="$row.x"
        :y="$row.y"
        :w="$row.width"
        :h="$row.height"
        :color="$row.fill"
        :rounded="$radius"
      >
        <Element
          x="0"
          y="0"
          :w="$markerWidth"
          :h="$row.height"
          :color="$row.markerColor"
          :rounded="$radius"
          :show="$row.active"
        />
        <Text
          :x="$row.labelX"
          :y="$row.labelY"
          :content="$row.label"
          :size="$labelSize"
          :color="$row.labelColor"
          maxlines="1"
        />
      </Element>
    </Element>
  `,
  state() {
    const layout = railLayout()
    return {
      width: RAIL_WIDTH,
      dividerX: RAIL_WIDTH - 2,
      pad: layout.pad,
      titleSize: layout.titleSize,
      labelSize: layout.labelSize,
      markerWidth: layout.markerWidth,
      radius: Math.round(TOKENS.radius.surface),
      background: hex(TOKENS.palette.background),
      border: hex(TOKENS.palette.hair),
      headingColor: hex(TOKENS.palette.inkMuted),
    }
  },
})
