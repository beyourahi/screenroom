/**
 * The same semantic tokens the desktop uses, at TV scale.
 *
 * The roles below are the ones `apps/desktop/src/design/mod.rs` defines, with the
 * same names and the same values: a platform may not silently fork the design
 * language, so anything the two disagree about has to be visible here rather than
 * hidden in a component.
 *
 * Two things *are* deliberately different, and both are viewing-distance
 * adaptations of the same roles rather than new ones:
 *
 * 1. **Type is larger.** The app is read from three metres, not sixty centimetres.
 *    The desktop scale is multiplied by `TYPE_SCALE`, and the floor test below keeps
 *    a body string from being drawn at a size nobody in the room can read.
 * 2. **Control heights are larger**, for the same reason, and because a remote's
 *    focus has to be unmistakable: `FOCUS_SCALE` on the focused card.
 *
 * What is *not* here is the private Dropout Design System's own palette. These are
 * the same neutral fallback values the desktop ships with, carrying the same
 * `provenance` string — and that string is shown in Settings, because a build whose
 * tokens are the fallback is not visually complete and must not pretend to be.
 */

/** A colour as a 32-bit `0xRRGGBBAA` integer, which is what the renderer takes. */
export type Color = number

function rgb(red: number, green: number, blue: number, alpha = 1): Color {
  const byte = (value: number): number => Math.round(Math.min(1, Math.max(0, value)) * 255)
  return (
    ((byte(red) << 24) | (byte(green) << 16) | (byte(blue) << 8) | byte(alpha)) >>> 0
  )
}

/** The desktop's root pixel size, which its whole scale is derived from. */
const ROOT_PX = 16

/** The viewing-distance multiplier. The one number a platform may differ by. */
export const TYPE_SCALE = 1.75

/** How much a focused card grows. A remote needs the change to be unmissable. */
export const FOCUS_SCALE = 1.04

/** Semantic surface and ink roles. Every field is a role, never a raw colour name. */
export interface Palette {
  /** The page ground. Black-first per the design system. */
  readonly background: Color
  /** First raised surface: cards, list rows. */
  readonly surface1: Color
  /** Second level: hover, inset panels. */
  readonly surface2: Color
  /** Third level: pressed, selected. */
  readonly surface3: Color
  /** Text on `background` and the surfaces. */
  readonly foreground: Color
  /** Secondary text that must still pass contrast on `background`. */
  readonly inkMuted: Color
  /** The one action accent. */
  readonly signal: Color
  /** Text on `signal`. */
  readonly signalForeground: Color
  /** Fill used for focus, in place of any ring or outline. */
  readonly focusSurface: Color
  /** Text on `focusSurface`. */
  readonly focusForeground: Color
  /** Destructive action fill. */
  readonly destructive: Color
  /** Text on `destructive`. */
  readonly destructiveForeground: Color
  /** Low-contrast hairline for separators between large regions. */
  readonly hair: Color
  /** Text for something unavailable. */
  readonly disabledForeground: Color
}

/** The design system's type scale, in pixels, at TV viewing distance. */
export interface TypeScale {
  readonly micro: number
  readonly caption: number
  readonly label: number
  readonly body: number
  readonly bodyLg: number
  readonly lead: number
  readonly subtitle: number
  readonly titleSm: number
  readonly title: number
  readonly titleLg: number
}

/** Corner radii, in pixels. `full` is a pill. */
export interface RadiusScale {
  readonly small: number
  readonly control: number
  readonly surface: number
  readonly overlay: number
  readonly full: number
}

/** Minimum interactive heights, in pixels. */
export interface ControlScale {
  readonly sm: number
  readonly md: number
  readonly lg: number
}

/** Spacing roles, named by purpose rather than by a numeric ramp. */
export interface Spacing {
  /** Outer padding of a screen shell. */
  readonly shellPad: number
  /** Padding inside a content panel. */
  readonly contentPad: number
  /** Gap between major sections. */
  readonly section: number
  /** Horizontal padding inside a field or button. */
  readonly fieldX: number
  /** Vertical padding inside a field or button. */
  readonly fieldY: number
}

/** The complete token set. */
export interface Tokens {
  readonly palette: Palette
  readonly type: TypeScale
  readonly radius: RadiusScale
  readonly control: ControlScale
  readonly spacing: Spacing
  /** Where these values came from; shown to the user, never guessed. */
  readonly provenance: string
}

/**
 * The neutral fallback tokens.
 *
 * These are plain neutral greys chosen to keep the app legible, **not** the design
 * system's palette. A build that shows this provenance is not visually complete.
 */
export const TOKENS: Tokens = {
  palette: {
    background: rgb(0.07, 0.07, 0.08),
    surface1: rgb(0.11, 0.11, 0.13),
    surface2: rgb(0.15, 0.15, 0.17),
    surface3: rgb(0.19, 0.19, 0.22),
    foreground: rgb(0.96, 0.96, 0.97),
    inkMuted: rgb(0.66, 0.66, 0.7),
    signal: rgb(0.99, 0.99, 1.0),
    signalForeground: rgb(0.08, 0.08, 0.09),
    focusSurface: rgb(0.98, 0.98, 1.0),
    focusForeground: rgb(0.06, 0.06, 0.07),
    destructive: rgb(0.85, 0.35, 0.35),
    destructiveForeground: rgb(0.06, 0.06, 0.07),
    hair: rgb(1.0, 1.0, 1.0, 0.1),
    disabledForeground: rgb(0.45, 0.45, 0.49),
  },
  type: {
    micro: 0.625 * ROOT_PX * TYPE_SCALE,
    caption: 0.75 * ROOT_PX * TYPE_SCALE,
    label: 0.875 * ROOT_PX * TYPE_SCALE,
    body: ROOT_PX * TYPE_SCALE,
    bodyLg: 1.0625 * ROOT_PX * TYPE_SCALE,
    lead: 1.125 * ROOT_PX * TYPE_SCALE,
    subtitle: 1.5 * ROOT_PX * TYPE_SCALE,
    titleSm: 1.875 * ROOT_PX * TYPE_SCALE,
    title: 2.25 * ROOT_PX * TYPE_SCALE,
    titleLg: 3.0 * ROOT_PX * TYPE_SCALE,
  },
  radius: {
    small: 0.5 * ROOT_PX,
    control: 0.75 * ROOT_PX,
    surface: 0.875 * ROOT_PX,
    overlay: ROOT_PX,
    full: 9999,
  },
  control: {
    // Larger than the desktop's, because the focused control has to be findable from
    // a sofa and the remote's own hit area is not the constraint a mouse pointer is.
    sm: 2.75 * ROOT_PX,
    md: 3.25 * ROOT_PX,
    lg: 3.75 * ROOT_PX,
  },
  spacing: {
    shellPad: 1.125 * ROOT_PX * TYPE_SCALE,
    contentPad: ROOT_PX * TYPE_SCALE,
    section: 4.0 * ROOT_PX * TYPE_SCALE,
    fieldX: 0.9375 * ROOT_PX * TYPE_SCALE,
    fieldY: 0.8125 * ROOT_PX * TYPE_SCALE,
  },
  provenance: 'public fallback (no design-system pin in this build)',
}

/**
 * The smallest body size this app will draw.
 *
 * From Samsung's own TV design guidance and the 10-foot rule of thumb: text below
 * roughly 24 logical pixels on a 1080p surface is not readable from a sofa. The
 * test keeps the scale above it, so a future edit to `TYPE_SCALE` cannot quietly make
 * every label illegible.
 */
export const MIN_LEGIBLE_PX = 24

/** A colour as a `#rrggbb` string, for the few places that need one. */
export function hex(color: Color): string {
  const value = color >>> 0
  const part = (shift: number): string =>
    (((value >>> shift) & 0xff) as number).toString(16).padStart(2, '0')
  return `#${part(24)}${part(16)}${part(8)}`
}
