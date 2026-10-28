/**
 * The Blits Vite plugin, which ships JavaScript with no types.
 *
 * The package publishes `@lightningjs/blits/vite` as a plain ESM module and declares no
 * types for it, so TypeScript cannot say what it exports. Declaring the one thing this
 * config uses is better than guessing at the plugin objects themselves: the array is
 * passed straight to Vite, which validates them, and a wrong guess here would fail the
 * build loudly rather than silently.
 */
declare module '@lightningjs/blits/vite' {
  import type { Plugin } from 'vite'

  /** The plugin array, in the order Blits expects. */
  const plugins: Plugin[]
  export default plugins
}
