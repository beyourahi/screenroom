# A Blits template only re-renders attributes that start with `:`

Date: 2026-10-28
Area: `apps/tv/src/screens/*`, `apps/tv/src/design/cards.ts`

## Symptom

On the television (and identically in a browser at 1920×1080) Home drew the single line
"Continue watching" at the top left of an otherwise black screen. Browse drew "Library".
Settings drew its row rectangles but no text at all. Nothing threw: no console message,
no `error`/`unhandledrejection` entry in the app's evidence log, and `ready()` ran to
completion (verified by logging each step of `Home.reload()`).

## Cause, part one: a bare `content="$x"` is read once, at creation

`@lightningjs/blits` 2.x decides reactivity with a single character:

```js
// node_modules/@lightningjs/blits/src/lib/codegenerator/generator.js
const isReactiveKey = (str) => str.startsWith(':')
```

A non-reactive attribute is put into the element config at build time and *never* gets an
`effect`, so `content="$late"` is evaluated once — when the node is created, which is
before `ready()` has set anything. Every screen's `subheading`, `footer`, `emptyTitle`,
`title`, `status` and row labels start as `''` and are filled in `ready()`, so they were
permanently empty. The heading worked only because its initial value was already correct.

Proof used at the time: a throwaway screen with `<Text content="$late">` and
`<Text :content="$late">` side by side, both set from `ready()` — the first stayed blank,
the second rendered.

Fix: every attribute whose value is a `$` expression now carries `:`
(`:content`, `:x`, `:y`, `:size`, `:color`, `:w`, `:h`, `:alpha`, `:scale`, `:rounded`).
`key="..."` on a `:for` stays bare on purpose.

## Cause, part two: a computed *object* is not a source of state keys

Three screens kept derived geometry on `computed: { grid() { return {...} } }` and then
referred to its members in the template as `$fieldX`, `$titleY`, `$footY`. A template
resolves `$name` against the component's **state** (plus props, computed names and loop
variables), so those were `undefined`: nodes were created with no position and no size.
Settings and Player were already correct because they copy their layout into state in an
`applyLayout()` before the first paint — Home, Browse and Detail now do the same.

## Guards added

- `apps/tv/test/templates.test.ts` parses each screen's template and every top-level key
  of its `state()`, `props` and `computed` blocks, plus `:for` variables, and fails on any
  `$name` that is not one of them. It also asserts its own parser found `cards`/`grid` in
  `Home.ts`, so the check cannot silently pass by finding nothing.
- `apps/tv/test/cards.test.ts` checks that a card is tall enough for the two lines it
  draws and that a whole page fits the band between the header and the footer.

## Layout numbers that were wrong because nobody could see the screen

- `titleStrip` was `body * 2.4` (67 px) while the title and sub-title under the art needed
  about 122 px, so the title was drawn past the card's own bottom edge. `gridFor` now
  derives the strip from the two lines (`titleGap + titleLine + subtitleLine`) and exposes
  `titleY`/`subtitleY` so a screen cannot re-derive them differently.
- A grid was sized from the width alone, so Browse's third row was drawn below the bottom
  of the set. `gridFor` now takes `{ top, bottom }` and shrinks the cards until the rows
  fit; Browse is 5×2 (10 per page) because 5×3 only fits at a type size nobody can read.

## Rule of thumb

If a value can change after the first frame, it needs `:` **and** it must live on the
component's state — not on a computed object, and not only on a loop item's nested shape.
