//! The Dropout Design System adapter for iced.
//!
//! ## Why this shape
//!
//! The private Dropout Studio Design System is a Svelte component library with a
//! DTCG token source. Its components cannot run in iced, so what transfers is its
//! **semantic token layer and interaction rules**. This module is the single place
//! in the desktop app that knows those semantics: widgets ask for a *role*
//! (`Level::Panel`, `action_button(focused)`), never for a hex value.
//!
//! ## Privacy
//!
//! The token *values* are private and must never be committed:
//!
//! * `examples/export_ds_tokens.rs` reads the pinned `tokens.json` from the private
//!   sibling checkout and writes `src/design/ds_pinned.rs` (Git-ignored).
//! * `build.rs` sets `screenroom_ds_pinned` only when that file exists.
//! * Without it — a fresh clone of the public repository — this module compiles
//!   against [`NEUTRAL`], a deliberately unbranded placeholder palette that is
//!   **not** the design system's look and must not be shipped as a final theme.
//!
//! [`provenance`] reports which of the two is in use so the app can say so plainly.

use iced::border::Radius;
use iced::widget::{button, container, text_input};
use iced::{Background, Border, Color, Shadow, Theme};

/// The design system defines its scale in `rem`; iced works in pixels.
const ROOT_PX: f32 = 16.0;

/// Builds a colour from sRGB components.
pub const fn color(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color { r, g, b, a }
}

/// Semantic surface and ink roles the app composes with.
///
/// Every field is a role, not a raw colour name, so a platform adapter can
/// re-map the same role without a widget knowing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// The page ground. Black-first per the design system.
    pub background: Color,
    /// First raised surface: rail cards, list rows.
    pub surface1: Color,
    /// Second level: hover, inset panels.
    pub surface2: Color,
    /// Third level: pressed, selected.
    pub surface3: Color,
    /// Text on `background` and the surfaces.
    pub foreground: Color,
    /// Secondary text that must still pass contrast on `background`.
    pub ink_muted: Color,
    /// The one action accent.
    pub signal: Color,
    /// Text on `signal`.
    pub signal_foreground: Color,
    /// Fill used for focus, in place of any ring or outline.
    pub focus_surface: Color,
    /// Text on `focus_surface`.
    pub focus_foreground: Color,
    /// Destructive action fill.
    pub destructive: Color,
    /// Text on `destructive`.
    pub destructive_foreground: Color,
    /// Low-contrast hairline for separators between large regions.
    pub hair: Color,
    /// Text for something unavailable.
    pub disabled_foreground: Color,
}

/// The design system's type scale, in pixels.
#[derive(Debug, Clone, Copy)]
pub struct TypeScale {
    /// Smallest label.
    pub micro: f32,
    /// Metadata under a title.
    pub caption: f32,
    /// Control and field labels.
    pub label: f32,
    /// Body copy.
    pub body: f32,
    /// Slightly larger body.
    pub body_lg: f32,
    /// Introductory paragraph.
    pub lead: f32,
    /// Card and section heading.
    pub subtitle: f32,
    /// Small screen title.
    pub title_sm: f32,
    /// Screen title.
    pub title: f32,
    /// Large screen title.
    pub title_lg: f32,
}

/// Corner radii, in pixels. `full` is a pill.
#[derive(Debug, Clone, Copy)]
pub struct RadiusScale {
    /// Chips and small choices.
    pub small: f32,
    /// Inputs and buttons.
    pub control: f32,
    /// Cards and panels.
    pub surface: f32,
    /// Overlays and dialogs.
    pub overlay: f32,
    /// Pill.
    pub full: f32,
}

/// Minimum interactive heights, in pixels.
#[derive(Debug, Clone, Copy)]
pub struct ControlScale {
    /// Compact control.
    pub sm: f32,
    /// Default control.
    pub md: f32,
    /// Prominent control.
    pub lg: f32,
}

/// Spacing roles. The design system names spacing by purpose, not by a numeric
/// ramp, so the app reuses those roles instead of inventing new steps.
#[derive(Debug, Clone, Copy)]
pub struct Spacing {
    /// Outer padding of a screen shell.
    pub shell_pad: f32,
    /// Padding inside a content panel.
    pub content_pad: f32,
    /// Gap between major sections.
    pub section: f32,
    /// Horizontal padding inside a field or button.
    pub field_x: f32,
    /// Vertical padding inside a field or button.
    pub field_y: f32,
}

/// The complete resolved token set.
#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    /// Colour roles.
    pub palette: Palette,
    /// Type scale.
    pub type_scale: TypeScale,
    /// Radii.
    pub radius: RadiusScale,
    /// Control heights.
    pub control: ControlScale,
    /// Spacing roles.
    pub spacing: Spacing,
    /// Where these values came from; shown to the user, never guessed.
    pub provenance: &'static str,
}

/// Placeholder tokens used when the private design-system pin is absent.
///
/// These are plain neutral greys chosen to keep the app legible, **not** the
/// design system's palette. A build that shows this provenance is not visually
/// complete and must not be presented as a finished Screenroom theme.
pub const NEUTRAL: Tokens = Tokens {
    palette: Palette {
        background: color(0.07, 0.07, 0.08, 1.0),
        surface1: color(0.11, 0.11, 0.13, 1.0),
        surface2: color(0.15, 0.15, 0.17, 1.0),
        surface3: color(0.19, 0.19, 0.22, 1.0),
        foreground: color(0.96, 0.96, 0.97, 1.0),
        ink_muted: color(0.66, 0.66, 0.70, 1.0),
        signal: color(0.99, 0.99, 1.00, 1.0),
        signal_foreground: color(0.08, 0.08, 0.09, 1.0),
        focus_surface: color(0.98, 0.98, 1.00, 1.0),
        focus_foreground: color(0.06, 0.06, 0.07, 1.0),
        destructive: color(0.85, 0.35, 0.35, 1.0),
        destructive_foreground: color(0.06, 0.06, 0.07, 1.0),
        hair: color(1.0, 1.0, 1.0, 0.10),
        disabled_foreground: color(0.45, 0.45, 0.49, 1.0),
    },
    type_scale: TypeScale {
        micro: 0.625 * ROOT_PX,
        caption: 0.75 * ROOT_PX,
        label: 0.875 * ROOT_PX,
        body: ROOT_PX,
        body_lg: 1.0625 * ROOT_PX,
        lead: 1.125 * ROOT_PX,
        subtitle: 1.5 * ROOT_PX,
        title_sm: 1.875 * ROOT_PX,
        title: 2.25 * ROOT_PX,
        title_lg: 3.0 * ROOT_PX,
    },
    radius: RadiusScale {
        small: 0.5 * ROOT_PX,
        control: 0.75 * ROOT_PX,
        surface: 0.875 * ROOT_PX,
        overlay: ROOT_PX,
        full: 9999.0,
    },
    control: ControlScale {
        sm: 2.25 * ROOT_PX,
        md: 2.5 * ROOT_PX,
        lg: 2.75 * ROOT_PX,
    },
    spacing: Spacing {
        shell_pad: 1.125 * ROOT_PX,
        content_pad: ROOT_PX,
        section: 6.0 * ROOT_PX,
        field_x: 0.9375 * ROOT_PX,
        field_y: 0.8125 * ROOT_PX,
    },
    provenance: "public fallback (no design-system pin in this build)",
};

#[cfg(screenroom_ds_pinned)]
#[path = "ds_pinned.rs"]
mod pinned;

/// The token set this build uses.
pub fn tokens() -> &'static Tokens {
    #[cfg(screenroom_ds_pinned)]
    {
        &pinned::PINNED
    }
    #[cfg(not(screenroom_ds_pinned))]
    {
        &NEUTRAL
    }
}

/// Where the active tokens came from, for an honest status line in the UI.
pub fn provenance() -> &'static str {
    tokens().provenance
}

/// How raised a container is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// The page ground.
    Page,
    /// A card or list row.
    Panel,
    /// A pressed or selected card.
    Raised,
    /// An overlay panel above the page.
    Overlay,
}

impl Level {
    /// The role's rest fill.
    fn fill(self) -> Color {
        let palette = &tokens().palette;
        match self {
            Self::Page => palette.background,
            Self::Panel => palette.surface1,
            Self::Raised => palette.surface2,
            Self::Overlay => palette.surface3,
        }
    }
}

/// A container style for `level`, or the focus fill when `focused`.
///
/// The design system forbids borders, outlines and rings on its own controls, so
/// focus is a *fill* change and it lands immediately: no transition, no glow.
pub fn container_style(level: Level, focused: bool) -> container::Style {
    let palette = &tokens().palette;
    container::Style {
        background: Some(Background::Color(if focused {
            palette.focus_surface
        } else {
            level.fill()
        })),
        text_color: Some(if focused {
            palette.focus_foreground
        } else {
            palette.foreground
        }),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: tokens().radius.surface.into(),
        },
        shadow: Shadow::default(),
        // Keep card edges on device pixels; a half-pixel surface reads as a blurry
        // edge on a 2x display.
        snap: true,
    }
}

/// A flat page container that fills its area.
pub fn page_style() -> container::Style {
    container_style(Level::Page, false)
}

/// A card or row: rest fill, a stronger fill on hover, and the focus fill when
/// the keyboard cursor is on it.
pub fn tile_style(focused: bool) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    move |_theme, status| {
        let palette = &tokens().palette;
        let (background, text_color) = match status {
            button::Status::Disabled => (palette.surface1, palette.disabled_foreground),
            _ if focused => (palette.focus_surface, palette.focus_foreground),
            button::Status::Hovered => (palette.surface2, palette.foreground),
            button::Status::Pressed => (palette.surface3, palette.foreground),
            button::Status::Active => (palette.surface1, palette.foreground),
        };
        button::Style {
            background: Some(Background::Color(background)),
            text_color,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: tokens().radius.surface.into(),
            },
            shadow: Shadow::default(),
            ..button::Style::default()
        }
    }
}

/// The action button: the accent fill at rest, the focus fill when focused.
pub fn action_style(focused: bool) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    move |_theme, status| {
        let palette = &tokens().palette;
        let (background, text_color) = match status {
            button::Status::Disabled => (palette.surface1, palette.disabled_foreground),
            _ if focused => (palette.focus_surface, palette.focus_foreground),
            button::Status::Hovered | button::Status::Pressed => {
                (palette.surface3, palette.foreground)
            }
            button::Status::Active => (palette.signal, palette.signal_foreground),
        };
        button::Style {
            background: Some(Background::Color(background)),
            text_color,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: tokens().radius.control.into(),
            },
            shadow: Shadow::default(),
            ..button::Style::default()
        }
    }
}

/// A low-emphasis button: surface fill, no border, focus fill when focused.
pub fn quiet_style(focused: bool) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    move |_theme, status| {
        let palette = &tokens().palette;
        let (background, text_color) = match status {
            button::Status::Disabled => (Color::TRANSPARENT, palette.disabled_foreground),
            _ if focused => (palette.focus_surface, palette.focus_foreground),
            button::Status::Hovered | button::Status::Pressed => {
                (palette.surface2, palette.foreground)
            }
            button::Status::Active => (palette.surface1, palette.foreground),
        };
        button::Style {
            background: Some(Background::Color(background)),
            text_color,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: tokens().radius.control.into(),
            },
            shadow: Shadow::default(),
            ..button::Style::default()
        }
    }
}

/// A destructive action: the danger role, focus fill when focused.
pub fn danger_style(focused: bool) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    move |_theme, status| {
        let palette = &tokens().palette;
        let (background, text_color) = match status {
            button::Status::Disabled => (palette.surface1, palette.disabled_foreground),
            _ if focused => (palette.focus_surface, palette.focus_foreground),
            _ => (palette.destructive, palette.destructive_foreground),
        };
        button::Style {
            background: Some(Background::Color(background)),
            text_color,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: tokens().radius.control.into(),
            },
            shadow: Shadow::default(),
            ..button::Style::default()
        }
    }
}

/// The search field: `surfaceInput` fill, no ring, focus fill on focus.
pub fn input_style() -> impl Fn(&Theme, text_input::Status) -> text_input::Style + 'static {
    move |_theme, status| {
        let palette = &tokens().palette;
        let focused = matches!(status, text_input::Status::Focused { .. });
        text_input::Style {
            background: Background::Color(if focused {
                palette.surface3
            } else {
                palette.surface1
            }),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: tokens().radius.control.into(),
            },
            icon: palette.ink_muted,
            placeholder: palette.ink_muted,
            value: palette.foreground,
            selection: palette.surface3,
        }
    }
}

/// A 1px separator colour for large regions.
pub fn hairline() -> Color {
    tokens().palette.hair
}

/// Rounds `value` to a radius, for ad-hoc containers.
pub fn radius_surface() -> Radius {
    tokens().radius.surface.into()
}

/// An iced theme derived from the same roles, so built-in widgets (slider,
/// checkbox, scrollbar) match the surfaces instead of fighting them.
pub fn theme() -> Theme {
    let palette = &tokens().palette;
    Theme::custom(
        "Screenroom".to_string(),
        iced::theme::Palette {
            background: palette.background,
            text: palette.foreground,
            primary: palette.signal,
            success: palette.focus_surface,
            danger: palette.destructive,
            // The design system defines no warning role; it uses `signal` for
            // emphasis, so the same role stands in rather than inventing a hue.
            warning: palette.signal,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_role_is_opaque_unless_it_is_a_hairline() {
        let palette = &tokens().palette;
        for (name, value) in [
            ("background", palette.background),
            ("surface1", palette.surface1),
            ("surface2", palette.surface2),
            ("surface3", palette.surface3),
            ("foreground", palette.foreground),
            ("ink_muted", palette.ink_muted),
            ("signal", palette.signal),
            ("focus_surface", palette.focus_surface),
            ("destructive", palette.destructive),
        ] {
            assert!(
                (value.a - 1.0).abs() < f32::EPSILON,
                "{name} must be fully opaque so surfaces composite predictably"
            );
        }
        assert!(
            palette.hair.a < 1.0,
            "the hairline role is intentionally soft"
        );
    }

    #[test]
    fn focus_is_never_the_rest_fill() {
        // The design system replaced rings with a fill change; a focus style that
        // equals the rest style would leave the keyboard cursor invisible.
        let palette = &tokens().palette;
        assert_ne!(palette.focus_surface, palette.surface1);
        assert_ne!(palette.focus_foreground, palette.foreground);
    }

    #[test]
    fn the_type_scale_is_ordered_and_legible() {
        let scale = &tokens().type_scale;
        let ordered = [
            scale.micro,
            scale.caption,
            scale.label,
            scale.body,
            scale.body_lg,
            scale.lead,
            scale.subtitle,
            scale.title_sm,
            scale.title,
            scale.title_lg,
        ];
        for pair in ordered.windows(2) {
            assert!(
                pair[0] < pair[1],
                "the type scale must increase monotonically"
            );
        }
        assert!(scale.caption >= 11.0, "caption text must stay readable");
    }

    #[test]
    fn control_heights_never_shrink_below_a_comfortable_target() {
        let control = &tokens().control;
        assert!(control.sm < control.md && control.md < control.lg);
        assert!(control.md >= 32.0);
    }

    #[test]
    fn provenance_is_always_reported() {
        assert!(!provenance().is_empty());
    }

    #[test]
    fn every_style_function_returns_a_style_for_every_status() {
        let statuses = [
            button::Status::Active,
            button::Status::Hovered,
            button::Status::Pressed,
            button::Status::Disabled,
        ];
        let theme = theme();
        for focused in [false, true] {
            for status in statuses {
                let tile = tile_style(focused)(&theme, status);
                let action = action_style(focused)(&theme, status);
                let quiet = quiet_style(focused)(&theme, status);
                let danger = danger_style(focused)(&theme, status);
                for style in [tile, action, quiet, danger] {
                    assert_eq!(style.border.width, 0.0, "controls must not draw borders");
                    assert!(style.background.is_some());
                }
            }
        }
    }

    #[test]
    fn a_focused_tile_uses_the_focus_fill_instead_of_the_rest_fill() {
        let theme = theme();
        let palette = &tokens().palette;
        let rest = tile_style(false)(&theme, button::Status::Active);
        let focused = tile_style(true)(&theme, button::Status::Active);
        assert_eq!(
            rest.background,
            Some(Background::Color(palette.surface1)),
            "an unfocused card sits on the first surface"
        );
        assert_eq!(
            focused.background,
            Some(Background::Color(palette.focus_surface)),
            "focus must be a stronger fill, not a ring"
        );
    }
}
