/**
 * Every `$name` a template names must exist on the component.
 *
 * This is a regression test for a defect that reached the television: three screens
 * kept their derived geometry on a *computed object* (`grid`, `metrics`, `layout`) and
 * referred to its members in the template as `$fieldX`, `$titleY`, `$footY` and so on.
 * A Blits template resolves `$name` against the component's state, so those were all
 * `undefined`; the nodes were still created, but with no position and no size, and the
 * screen drew one header line on an otherwise black surface. Nothing threw, no console
 * message appeared, and the TypeScript compiler cannot see inside a template string —
 * which is why the guard has to be here rather than in the type checker.
 *
 * The check is deliberately structural rather than clever: parse each screen's template
 * for `$identifier`, collect the top-level keys of its `state()`, `props` and `computed`
 * blocks plus its `:for` loop variables, and require every reference to be one of those
 * or one of the handful of members Blits itself puts on a component.
 */

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readdirSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', 'src')

/**
 * Every directory that holds a component with a template.
 *
 * Screens were the first; the shared rail lives in `components/` and its template has
 * the same contract, so it is checked by the same parser rather than trusted.
 */
const DIRECTORIES = ['screens', 'components']

/**
 * Members Blits provides to every component, which no screen declares.
 *
 * `$focus` (the focus ref), `$router`, `$log`, `$setTimeout`/`$clearTimeout` and
 * `$fireAncestors` are framework surface; anything else a template names has to come
 * from the component itself.
 */
const FRAMEWORK_MEMBERS = new Set([
  'focus',
  'router',
  'log',
  'setTimeout',
  'clearTimeout',
  'fireAncestors',
  'emit',
  'el',
  'app',
  'props',
  'route',
  'intercept',
])

/** The component files, in a stable order. */
function componentFiles(): { readonly dir: string; readonly name: string }[] {
  return DIRECTORIES.flatMap((dir) =>
    readdirSync(join(ROOT, dir))
      .filter((name) => name.endsWith('.ts'))
      .sort()
      .map((name) => ({ dir, name })),
  )
}

/** The source of one component file, comments removed. */
function sourceOf(dir: string, name: string): string {
  return withoutComments(readFileSync(join(ROOT, dir, name), 'utf8'))
}

/** The source with its comments removed, so a prose line cannot look like a key. */
function withoutComments(source: string): string {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/(^|[^:'"\\])\/\/[^\n]*/g, '$1')
}

/** The body of the component's `template` string literal. */
function templateBody(source: string): string {
  const marker = 'template: `'
  const start = source.indexOf(marker)
  assert.notEqual(start, -1, 'the component has no template')
  const from = start + marker.length
  const end = source.indexOf('`', from)
  assert.notEqual(end, -1, 'the template string is not closed')
  return source.slice(from, end)
}

/** The index of the brace that closes the block opened at `open`. */
function closingBrace(source: string, open: number): number {
  let depth = 0
  for (let i = open; i < source.length; i += 1) {
    const char = source[i]
    if (char === '{') {
      depth += 1
    } else if (char === '}') {
      depth -= 1
      if (depth === 0) {
        return i
      }
    }
  }
  return -1
}

/** The body of the block introduced by `signature`, or `null` when it is absent. */
function blockBody(source: string, signature: string): string | null {
  const at = source.indexOf(signature)
  if (at === -1) {
    return null
  }
  const open = source.indexOf('{', at + signature.length - 1)
  if (open === -1) {
    return null
  }
  const close = closingBrace(source, open)
  return close === -1 ? null : source.slice(open, close + 1)
}

/** How many brackets one line opens, ignoring brackets inside a string. */
function bracketDelta(line: string): number {
  let delta = 0
  let quote: string | null = null
  for (let i = 0; i < line.length; i += 1) {
    const char = line[i]
    if (quote !== null) {
      if (char === quote) {
        quote = null
      }
      continue
    }
    if (char === "'" || char === '"' || char === '`') {
      quote = char
    } else if (char === '{' || char === '(' || char === '[') {
      delta += 1
    } else if (char === '}' || char === ')' || char === ']') {
      delta -= 1
    }
  }
  return delta
}

/**
 * The keys declared at the top level of one object or class body.
 *
 * Both spellings count: `key: value` in `state` and `props`, and `name() {}` in
 * `computed`. Anything nested (a prop's own options, a function body) sits at a deeper
 * level and is therefore not a key of the component.
 */
function topLevelKeys(block: string): string[] {
  const keys: string[] = []
  let depth = 0
  for (const line of block.split('\n')) {
    const trimmed = line.trim()
    if (depth === 1 && trimmed.length > 0) {
      const match = /^([A-Za-z_$][\w$]*)\s*[:(]/.exec(trimmed)
      if (match !== null && match[1] !== undefined) {
        keys.push(match[1])
      }
    }
    depth += bracketDelta(line)
  }
  return keys
}

/** The keys a component declares in `state`, `props` and `computed`. */
function declaredKeys(source: string): Set<string> {
  const keys = new Set<string>()
  // `state()` returns its object, so the block to read starts at that object.
  const stateAt = source.indexOf('state()')
  if (stateAt !== -1) {
    const returnAt = source.indexOf('return {', stateAt)
    if (returnAt !== -1) {
      const open = source.indexOf('{', returnAt)
      const close = closingBrace(source, open)
      if (close !== -1) {
        for (const key of topLevelKeys(source.slice(open, close + 1))) {
          keys.add(key)
        }
      }
    }
  }
  for (const signature of ['props:', 'computed:']) {
    const body = blockBody(source, signature)
    if (body !== null) {
      for (const key of topLevelKeys(body)) {
        keys.add(key)
      }
    }
  }
  return keys
}

/** The `:for` variables a template introduces, which are written as `$name` too. */
function loopVariables(template: string): Set<string> {
  const names = new Set<string>()
  for (const match of template.matchAll(/:?for="([A-Za-z_$][\w$]*)\s+in\b/g)) {
    if (match[1] !== undefined) {
      names.add(match[1])
    }
  }
  return names
}

/** Every `$identifier` base the template refers to. */
function references(template: string): Set<string> {
  const names = new Set<string>()
  for (const match of template.matchAll(/\$([A-Za-z_$][\w$]*)/g)) {
    if (match[1] !== undefined) {
      names.add(match[1])
    }
  }
  return names
}

test('every template reference in every component resolves to something it declares', () => {
  const problems: string[] = []
  for (const { dir, name } of componentFiles()) {
    const source = sourceOf(dir, name)
    const template = templateBody(source)
    const known = new Set([
      ...declaredKeys(source),
      ...loopVariables(template),
      ...FRAMEWORK_MEMBERS,
    ])
    const missing = [...references(template)].filter((name) => !known.has(name)).sort()
    if (missing.length > 0) {
      problems.push(`${name}: ${missing.map((key) => `$${key}`).join(', ')}`)
    }
  }
  assert.deepEqual(
    problems,
    [],
    `a template can only read state, props, computed values and its own loop variables:\n${problems.join('\n')}`,
  )
})

/**
 * A `:for` variable must not be called `key`.
 *
 * This is the second regression test taken from the television. The search screen's
 * keyboard loop was written `:for="key in $keys"` with `key="$key.id"`, which reads
 * naturally and compiles to a collision: Blits assigns the loop variable to the scope
 * slot named after it, and then the `key` attribute assigns the id to the *same* slot.
 * The loop body therefore saw a string where it expected an object, every key was
 * created with an undefined size and position, and the keyboard drew nothing at all —
 * no error, no warning, an empty half-screen beside a correct header.
 */
test('no loop variable is named key, which the compiled scope overwrites', () => {
  const problems: string[] = []
  for (const { dir, name } of componentFiles()) {
    const template = templateBody(sourceOf(dir, name))
    for (const match of template.matchAll(/:?for="([A-Za-z_$][\w$]*)\s+in\b/g)) {
      if (match[1] === 'key') {
        problems.push(`${name}: a :for variable is named key`)
      }
    }
  }
  assert.deepEqual(problems, [], problems.join('\n'))
})

test('the checker itself sees the state it is supposed to see', () => {
  // A guard that silently found nothing would pass forever. `Home` declares `cards` and
  // computes `grid`, so both must come back from the parser.
  const source = sourceOf('screens', 'Home.ts')
  const declared = declaredKeys(source)
  for (const key of ['cards', 'background', 'footY', 'grid']) {
    assert.ok(declared.has(key), `expected ${key} to be parsed out of Home.ts`)
  }
  // And the template must really mention them, or the check above is vacuous.
  const named = references(templateBody(source))
  assert.ok(named.has('footY'), 'expected the Home template to name $footY')
  assert.deepEqual(
    [...loopVariables(templateBody(source))],
    ['card'],
    'expected the Home template to declare the card loop variable',
  )
})
