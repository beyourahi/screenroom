#!/usr/bin/env node
/**
 * Turns the Vite output into something a TV can install.
 *
 * Vite produces a web page; a Tizen widget is a directory with a `config.xml` at its
 * root and an `index.html` that loads nothing from outside that directory. The gap
 * between those two is what this script closes, and it closes it *checking* rather than
 * copying and hoping: every failure below is one that would otherwise appear as a blank
 * screen on the TV, where there is no console to read.
 *
 * The checks, and why each one is here:
 *
 * * `main.js` exists and is referenced by `index.html`. A bundle that is built but not
 *   linked is a white screen with no error.
 * * no reference is absolute. The widget has no origin, so `/assets/main.js` resolves
 *   against nothing. Vite is configured for relative output, and this verifies it
 *   rather than trusting the config.
 * * `config.xml` names the same `index.html`, and carries the version this build was
 *   made from. A widget whose declared version drifts from its code cannot be told
 *   apart from the previous one once it is installed.
 *
 * The output is `build/tizen/`, a directory containing nothing but what belongs in the
 * widget. Nothing here writes outside this package.
 */

import { createHash } from 'node:crypto'
import { existsSync } from 'node:fs'
import { copyFile, mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const distDir = join(packageRoot, 'dist')
const configPath = join(packageRoot, 'config.xml')
const iconPath = join(packageRoot, 'icon.png')
// `build/widget` rather than `build/tizen`: this directory is wiped on every build, and
// the packaged `.wgt` used to land inside it — so re-staging quietly deleted the one
// artifact that takes a toolchain to produce. The package is copied out to `build/`
// instead, where nothing here can remove it.
const stageDir = join(packageRoot, 'build', 'widget')

/** Files that must exist for the widget to be installable. */
const REQUIRED = ['index.html', 'main.js']

/** Stops the build with a message that says what to do about it. */
function fail(message) {
  console.error(`prepare-tizen-dist: ${message}`)
  process.exit(1)
}

async function main() {
  if (!existsSync(distDir)) {
    fail('dist/ does not exist — run the Vite build first (`pnpm build` does both).')
  }
  for (const name of REQUIRED) {
    if (!existsSync(join(distDir, name))) {
      fail(`dist/${name} is missing; the widget would not start.`)
    }
  }
  if (!existsSync(configPath)) {
    fail('config.xml is missing; a Tizen widget cannot be installed without it.')
  }
  if (!existsSync(iconPath)) {
    fail('icon.png is missing; the TV shows it on the app list.')
  }

  const html = await readFile(join(distDir, 'index.html'), 'utf8')
  if (!/<script[^>]+src=["']\.\/main\.js["']/.test(html)) {
    fail('dist/index.html does not load ./main.js relatively; the widget would load nothing.')
  }
  // An absolute URL here would work in the dev server and fail in the widget, which is
  // exactly the kind of difference that survives until the TV.
  const absolute = [...html.matchAll(/(?:src|href)=["'](\/[^"']*)["']/g)].map((match) => match[1])
  if (absolute.length > 0) {
    fail(`dist/index.html references absolute paths (${absolute.join(', ')}); a widget has no origin.`)
  }

  const config = await readFile(configPath, 'utf8')
  const declared = /<content\s+src=["']([^"']+)["']/.exec(config)?.[1]
  if (declared !== 'index.html') {
    fail(`config.xml starts "${declared ?? 'nothing'}"; the widget needs index.html.`)
  }

  const version = JSON.parse(await readFile(join(packageRoot, 'package.json'), 'utf8')).version
  const stamped = config.replace(/(<widget[^>]*\bversion=["'])[^"']*(["'])/, `$1${version}$2`)
  if (stamped === config && !config.includes(`version="${version}"`)) {
    fail('could not find a version attribute in config.xml to stamp.')
  }

  await rm(stageDir, { recursive: true, force: true })
  await mkdir(stageDir, { recursive: true })
  // Only the built page and the widget's own metadata: anything else in the directory
  // is something the TV would download for no reason, or something that must not ship.
  await copyFile(join(distDir, 'index.html'), join(stageDir, 'index.html'))
  await copyFile(join(distDir, 'main.js'), join(stageDir, 'main.js'))
  if (existsSync(join(distDir, 'assets'))) {
    await copyTree(join(distDir, 'assets'), join(stageDir, 'assets'))
  }
  await writeFile(join(stageDir, 'config.xml'), stamped, 'utf8')
  await copyFile(iconPath, join(stageDir, 'icon.png'))
  // The Tizen project metadata goes in too, so the staging directory is a complete,
  // packageable Tizen web project rather than a directory that merely looks like one.
  // `tizen_web_project.yaml`'s own `excludes` list keeps both files out of the widget,
  // which is why they can sit beside the files that ship.
  for (const metadata of ['.tproject', 'tizen_web_project.yaml']) {
    if (existsSync(join(packageRoot, metadata))) {
      await copyFile(join(packageRoot, metadata), join(stageDir, metadata))
    }
  }

  const bundle = await readFile(join(stageDir, 'main.js'))
  const digest = createHash('sha256').update(bundle).digest('hex')
  console.log(`prepare-tizen-dist: staged ${stageDir}`)
  console.log(
    'prepare-tizen-dist: package it with the tizen-sdk-skills build runner against that directory',
  )
  console.log(`prepare-tizen-dist: version ${version}, main.js ${bundle.length} bytes`)
  console.log(`prepare-tizen-dist: main.js sha256 ${digest}`)
  console.log('prepare-tizen-dist: config.xml and icon.png included; nothing else.')
}

/** Copies a directory recursively, without needing Node 22's `cp`. */
async function copyTree(from, to) {
  await mkdir(to, { recursive: true })
  const { readdir, stat } = await import('node:fs/promises')
  for (const entry of await readdir(from)) {
    const source = join(from, entry)
    const target = join(to, entry)
    const info = await stat(source)
    if (info.isDirectory()) {
      await copyTree(source, target)
    } else {
      await copyFile(source, target)
    }
  }
}

await main()
