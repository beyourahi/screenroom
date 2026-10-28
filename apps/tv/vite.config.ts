import blits from '@lightningjs/blits/vite'
import { defineConfig } from 'vite'

/**
 * The TV build.
 *
 * Three settings here are TV-specific rather than taste, and each is the difference
 * between a bundle that loads on the TV and one that does not:
 *
 * * **The target is the installed engine, not the newest browser.** A 2021 AU8000 runs
 *   a Tizen 6.0 web engine. Emitting syntax that engine cannot parse produces a white
 *   screen with no error the app can report — so the target is pinned below the
 *   engine's ceiling on purpose.
 * * **One chunk.** A widget loads from `file://`-style local storage with no server, so
 *   a second request for a split chunk is a request that can fail. `inlineDynamicImports`
 *   plus a fixed asset name keeps the output to one script.
 * * **Relative base.** The widget has no origin, so absolute `/assets/...` URLs point
 *   nowhere. `base: './'` is what makes the built bundle load.
 *
 * `emptyOutDir` is left on so a stale file from an earlier build cannot be packaged
 * into the widget by mistake.
 *
 * **`import.meta.url` is defined away, and that is not cosmetic.** The widget loads from
 * a `file://` origin, where Chromium blocks `type="module"` scripts for CORS reasons —
 * the script never runs and the app paints nothing, with no error a screen could show.
 * So the staged page loads the bundle as a classic deferred script instead (see
 * `scripts/prepare-tizen-dist.mjs`). `import.meta` is a *parse-time* error outside a
 * module, so the one occurrence inside Blits' version probe has to be replaced before
 * the bundle can be loaded that way at all.
 */
/**
 * Replaces the one module-only expression in the bundle.
 *
 * `define` does not rewrite `import.meta.url`, and the staging step refuses to package
 * a bundle containing it, so it is replaced here — in `renderChunk`, after everything
 * is bundled and before the file is written. `location.href` is the same value for a
 * widget-loaded script, and the surrounding code is a guarded version probe whose
 * `catch` already handles being unable to answer.
 */
const classicBundle = {
  name: 'screenroom-tv-classic-bundle',
  renderChunk(code: string) {
    return code.includes('import.meta') ? code.replace(/import\.meta\.url/g, 'window.location.href') : code
  },
}

export default defineConfig({
  base: './',
  plugins: [...blits, classicBundle],
  build: {
    target: 'es2019',
    outDir: 'dist',
    emptyOutDir: true,
    // A widget has no server to fetch extra chunks from.
    assetsInlineLimit: Number.MAX_SAFE_INTEGER,
    cssCodeSplit: false,
    sourcemap: false,
    rollupOptions: {
      output: {
        entryFileNames: 'main.js',
        assetFileNames: 'assets/[name][extname]',
        // Lightning and Blits must end up in the same file as the app: a dynamic
        // import that resolves lazily would be a second request the widget cannot make.
        inlineDynamicImports: true,
        manualChunks: undefined,
      },
    },
  },
  server: {
    port: 5175,
    host: '127.0.0.1',
  },
})
