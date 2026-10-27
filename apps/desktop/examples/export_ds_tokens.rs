//! Exports the pinned Dropout Design System semantic tokens into a Rust module.
//!
//! ```bash
//! cargo run -p screenroom-desktop --example export_ds_tokens
//! cargo run -p screenroom-desktop --example export_ds_tokens -- /path/to/tokens.json
//! ```
//!
//! It writes `apps/desktop/src/design/ds_pinned.rs`, which `.gitignore` excludes,
//! and records the design system's commit hash inside the file so drift between
//! the pinned values and the private upstream is detectable.
//!
//! Only the semantic roles the desktop app consumes are exported. Values are
//! resolved from their DTCG aliases (`{color.ink.950}`) and converted from OKLCH
//! to the sRGB encoding iced expects. Nothing is copied into a committed file.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// Root font size the design system's `rem` scale resolves against.
const ROOT_PX: f64 = 16.0;

/// Default private checkout, used when no path argument or env var is given.
const DEFAULT_TOKENS: &str = "dropout-design-system/src/lib/tokens/tokens.json";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tokens_path = match std::env::args().nth(1) {
        Some(path) => PathBuf::from(path),
        None => {
            let from_env = std::env::var_os("SCREENROOM_DS_TOKENS").map(PathBuf::from);
            match from_env {
                Some(path) => path,
                None => default_tokens_path()?,
            }
        }
    };
    if !tokens_path.is_file() {
        return Err(format!(
            "{} is not a file. Pass the private tokens.json path explicitly.",
            tokens_path.display()
        )
        .into());
    }

    let raw = std::fs::read_to_string(&tokens_path)?;
    let tokens: Value = serde_json::from_str(&raw)?;
    let revision = design_system_revision(&tokens_path);

    let resolver = Resolver { tokens: &tokens };
    let dark = tokens
        .get("semantic")
        .and_then(|semantic| semantic.get("dark"))
        .ok_or("tokens.json has no `semantic.dark` group; the design system changed shape")?;

    // Role → DTCG path. A missing role is an error, never a silent default, so a
    // design-system change stops the export instead of shipping wrong colours.
    let mut palette = Vec::new();
    for (field, role) in [
        ("background", "background"),
        ("surface1", "surface1"),
        ("surface2", "surface2"),
        ("surface3", "surface3"),
        ("foreground", "foreground"),
        ("ink_muted", "inkMuted"),
        ("signal", "signal"),
        // The design system defines no `signalForeground`. `focusForeground` is
        // the same contrast situation — text on a near-white fill in the dark
        // theme — so it is reused rather than invented.
        ("signal_foreground", "focusForeground"),
        ("focus_surface", "focusSurface"),
        ("focus_foreground", "focusForeground"),
        ("destructive", "destructive"),
        ("destructive_foreground", "destructiveForeground"),
        ("hair", "hair"),
        ("disabled_foreground", "disabledForeground"),
    ] {
        let raw_value = dark
            .get(role)
            .ok_or_else(|| format!("semantic.dark.{role} is missing"))?;
        let rgba = resolver.color(raw_value)?;
        palette.push(format!(
            "        {field}: color({:.4}, {:.4}, {:.4}, {:.4}),",
            rgba.0, rgba.1, rgba.2, rgba.3
        ));
    }

    let type_scale = scale(
        &tokens,
        "typeScale",
        ROOT_PX,
        &[
            "micro", "caption", "label", "body", "bodyLg", "lead", "subtitle", "titleSm", "title",
            "titleLg",
        ],
    )?;
    let radius = scale(
        &tokens,
        "radius",
        ROOT_PX,
        &["small", "control", "surface", "overlay", "full"],
    )?;
    let control = scale(&tokens, "control", ROOT_PX, &["sm", "md", "lg"])?;
    let spacing = scale(
        &tokens,
        "space",
        ROOT_PX,
        &["shellPad", "contentPad", "section", "fieldX", "fieldY"],
    )?;

    let provenance = match &revision {
        Some(revision) => format!("dropout-design-system@{revision} semantic.dark"),
        None => "dropout-design-system (revision unknown) semantic.dark".to_string(),
    };

    let mut out = String::new();
    out.push_str(
        "//! Generated from the private Dropout Design System. DO NOT COMMIT.\n\
         //!\n\
         //! Regenerate with:\n\
         //! `cargo run -p screenroom-desktop --example export_ds_tokens`\n\
         //!\n",
    );
    out.push_str(&format!("//! Source: {}\n", tokens_path.display()));
    out.push_str(&format!("//! Pinned at: {provenance}\n\n"));
    out.push_str(
        "use crate::design::{ControlScale, Palette, RadiusScale, Spacing, Tokens, TypeScale, color};\n\n\
         /// Semantic Dropout Design System tokens resolved for iced.\n\
         pub const PINNED: Tokens = Tokens {\n    palette: Palette {\n",
    );
    for line in &palette {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("    },\n");
    out.push_str(&format!(
        "    type_scale: TypeScale {{\n{}\n    }},\n",
        type_scale
    ));
    out.push_str(&format!(
        "    radius: RadiusScale {{\n{}\n    }},\n",
        radius
    ));
    out.push_str(&format!(
        "    control: ControlScale {{\n{}\n    }},\n",
        control
    ));
    out.push_str(&format!("    spacing: Spacing {{\n{}\n    }},\n", spacing));
    out.push_str(&format!("    provenance: \"{provenance}\",\n}};\n"));

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/design/ds_pinned.rs");
    std::fs::write(&target, out)?;
    println!("wrote {}", target.display());
    println!("provenance: {provenance}");
    println!(
        "This file is Git-ignored. Rebuild to pick it up (`cargo build -p screenroom-desktop`)."
    );
    Ok(())
}

/// The private checkout's default location relative to the projects folder.
fn default_tokens_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    let path = PathBuf::from(home)
        .join("Desktop/projects")
        .join(DEFAULT_TOKENS);
    if path.is_file() {
        return Ok(path);
    }
    Err(format!(
        "no tokens.json at {}; pass a path or set SCREENROOM_DS_TOKENS",
        path.display()
    )
    .into())
}

/// The design system's commit, read from its own checkout when possible.
fn design_system_revision(tokens_path: &Path) -> Option<String> {
    let repo = tokens_path
        .parent()?
        .ancestors()
        .find(|candidate| candidate.join(".git").exists())?;
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Resolves DTCG aliases (`{color.ink.950}`) down to a concrete token.
struct Resolver<'a> {
    tokens: &'a Value,
}

impl Resolver<'_> {
    /// Resolves a token object (or alias string) to an sRGB quadruple.
    fn color(&self, value: &Value) -> Result<(f64, f64, f64, f64), Box<dyn std::error::Error>> {
        let resolved = self.resolve(value, 0)?;
        let object = resolved
            .as_object()
            .ok_or_else(|| format!("expected a colour token, found {resolved}"))?;
        let alpha = object.get("alpha").and_then(Value::as_f64).unwrap_or(1.0);
        let space = object
            .get("colorSpace")
            .and_then(Value::as_str)
            .ok_or("colour token has no colorSpace")?;
        let components = object
            .get("components")
            .and_then(Value::as_array)
            .ok_or("colour token has no components")?;
        let numbers: Vec<f64> = components
            .iter()
            .map(|component| component.as_f64().unwrap_or(0.0))
            .collect();
        match space {
            "oklch" => {
                let (l, c, h) = (
                    numbers.first().copied().unwrap_or(0.0),
                    numbers.get(1).copied().unwrap_or(0.0),
                    numbers.get(2).copied().unwrap_or(0.0),
                );
                let (r, g, b) = oklch_to_srgb(l, c, h);
                Ok((r, g, b, alpha))
            }
            "srgb" => Ok((
                numbers.first().copied().unwrap_or(0.0),
                numbers.get(1).copied().unwrap_or(0.0),
                numbers.get(2).copied().unwrap_or(0.0),
                alpha,
            )),
            other => Err(format!("unsupported colour space {other:?}").into()),
        }
    }

    /// Follows `{group.token}` references until a concrete value is reached.
    fn resolve(&self, value: &Value, depth: usize) -> Result<Value, Box<dyn std::error::Error>> {
        if depth > 8 {
            return Err("token alias chain is too deep".into());
        }
        if let Some(alias) = value.as_str()
            && let Some(path) = alias
                .strip_prefix('{')
                .and_then(|rest| rest.strip_suffix('}'))
        {
            let mut current = self.tokens;
            for segment in path.split('.') {
                current = current
                    .get(segment)
                    .ok_or_else(|| format!("alias {alias} does not resolve"))?;
            }
            let inner = current
                .get("$value")
                .cloned()
                .unwrap_or_else(|| current.clone());
            return self.resolve(&inner, depth + 1);
        }
        if let Some(inner) = value.get("$value") {
            return self.resolve(inner, depth + 1);
        }
        Ok(value.clone())
    }
}

/// Formats one scale group as Rust field initialisers.
fn scale(
    tokens: &Value,
    group: &str,
    root_px: f64,
    fields: &[&str],
) -> Result<String, Box<dyn std::error::Error>> {
    let node = tokens
        .get(group)
        .ok_or_else(|| format!("tokens.json has no `{group}` group"))?;
    let mut lines = String::new();
    for field in fields {
        let entry = node
            .get(field)
            .ok_or_else(|| format!("{group}.{field} is missing"))?;
        // DTCG wraps the payload in `$value`; the shape inside is `{ value, unit }`.
        let entry = entry.get("$value").unwrap_or(entry);
        // Covers both `{ value, unit }` entries and bare numbers.
        let value = entry.get("value").unwrap_or(entry);
        let unit = entry.get("unit").and_then(Value::as_str).unwrap_or("px");
        let number = value
            .as_f64()
            .ok_or_else(|| format!("{group}.{field} is not a number"))?;
        let pixels = if unit == "rem" {
            number * root_px
        } else {
            number
        };
        // A `fieldX` style name becomes `field_x`.
        let rust_name: String = field
            .chars()
            .map(|character| {
                if character.is_ascii_uppercase() {
                    format!("_{}", character.to_ascii_lowercase())
                } else {
                    character.to_string()
                }
            })
            .collect();
        lines.push_str(&format!("        {rust_name}: {pixels:.4},\n"));
    }
    Ok(lines.trim_end().to_string())
}

/// Converts OKLCH to gamma-encoded sRGB, clamped to the displayable gamut.
fn oklch_to_srgb(l: f64, c: f64, h_degrees: f64) -> (f64, f64, f64) {
    let h = h_degrees.to_radians();
    let a = c * h.cos();
    let b = c * h.sin();

    let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = l - 0.089_484_177_5 * a - 1.291_485_548_0 * b;

    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);

    let r = 4.076_741_662_1 * l3 - 3.307_711_591_3 * m3 + 0.230_969_929_2 * s3;
    let g = -1.268_438_004_6 * l3 + 2.609_757_401_1 * m3 - 0.341_319_396_5 * s3;
    let bl = -0.004_196_086_3 * l3 - 0.703_418_614_7 * m3 + 1.707_614_701_0 * s3;

    (encode(clamp01(r)), encode(clamp01(g)), encode(clamp01(bl)))
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

/// Linear light to sRGB transfer function.
fn encode(value: f64) -> f64 {
    if value <= 0.003_130_8 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}
