/**
 * The application shell.
 *
 * It owns three things and nothing else: the route table, the one key that is global
 * rather than per screen (Back), and the startup probe that decides what this host can
 * do. Screens draw themselves.
 *
 * Back is handled through `intercept` rather than through the router alone, because the
 * two cases have different remedies. With history, Back pops — the router does that.
 * At the home screen there is nothing to pop, and a Back key that does nothing is how
 * an app traps somebody on its front page. So: at home, leave the app, the way every
 * other TV app does.
 */

import Blits from '@lightningjs/blits'

import { hex, TOKENS } from './design/tokens.ts'
import { prepare } from './app/runtime.ts'
import { exitApp } from './platform/tizen.ts'
import Browse from './screens/Browse.ts'
import Detail from './screens/Detail.ts'
import Home from './screens/Home.ts'
import Player from './screens/Player.ts'
import Search from './screens/Search.ts'
import Settings from './screens/Settings.ts'
import Sources from './screens/Sources.ts'

/** True when a key event is the remote's Back key, in either spelling it arrives as. */
function isBackKey(keyCode: number): boolean {
  // 10009 is the Samsung remote's Return key; 8 is Backspace, which is what the key
  // arrives as in a desktop browser preview.
  return keyCode === 10009 || keyCode === 8
}

export default Blits.Application({
  state() {
    return {
      background: hex(TOKENS.palette.background),
    }
  },
  template: `
    <Element w="1920" h="1080" :color="$background">
      <RouterView w="1920" h="1080" />
    </Element>
  `,
  router: {
    // The routes the rail opens, plus the two leaf screens (a file, and the player).
    // The `data` on Movies and TV Shows is the prop that decides which rows Browse
    // draws; the router passes route data straight through as props.
    routes: [
      { path: '/', component: Home },
      { path: '/movies', component: Browse, data: { kind: 'movies' } },
      { path: '/series', component: Browse, data: { kind: 'series' } },
      { path: '/search', component: Search },
      { path: '/sources', component: Sources },
      { path: '/detail', component: Detail },
      { path: '/settings', component: Settings },
      { path: '/player', component: Player },
    ],
    // Back pops the router's own history. The only case it cannot express — nothing to
    // pop — is what `intercept` below handles.
    backNavigation: true,
  },
  hooks: {
    ready() {
      // Probes the host and requests the remote's extra keys exactly once, before any
      // screen can ask what this host supports.
      prepare()
    },
  },
  input: {
    /**
     * The root-level key hook, called before the focused component sees the event.
     *
     * Returning the event lets it through; returning nothing swallows it. Only the one
     * case that the router cannot express is swallowed here, so the arrow keys, Enter
     * and everything else reach their screen untouched.
     */
    intercept(event) {
      if (!isBackKey(event.keyCode)) {
        return event
      }
      const path = this.$router.currentRoute.path
      if (path !== '/') {
        return event
      }
      // Home has nothing to go back to. Leaving the app is what the user asked for;
      // in a browser preview there is no app to leave, and saying so beats a dead key.
      if (!exitApp()) {
        this.$log.info('Screenroom: at the home screen with nowhere to go back to.')
      }
      return undefined
    },
  },
})
