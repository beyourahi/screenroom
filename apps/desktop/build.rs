//! Decides whether the desktop crate is built against a locally generated
//! Dropout Design System token pin.
//!
//! The private design system's token *values* must never enter the repository.
//! `examples/export_ds_tokens.rs` reads the pinned `tokens.json` from the private
//! sibling checkout and writes `src/design/ds_pinned.rs`, which `.gitignore`
//! excludes. This script only checks whether that file is present:
//!
//! * present → `screenroom_ds_pinned` is set and the real semantic tokens are used,
//! * absent → the crate uses the neutral public fallback in `design/mod.rs`, so a
//!   clone of the public repository builds with no private source.
//!
//! Nothing here writes, copies or publishes private data.

use std::path::Path;

fn main() {
    // A custom cfg must be declared or `unexpected_cfgs` (denied under
    // `clippy -D warnings`) fails the build.
    println!("cargo:rustc-check-cfg=cfg(screenroom_ds_pinned)");
    println!("cargo:rerun-if-changed=build.rs");

    let pinned = Path::new("src/design/ds_pinned.rs");
    println!("cargo:rerun-if-changed=src/design/ds_pinned.rs");
    if pinned.exists() {
        println!("cargo:rustc-cfg=screenroom_ds_pinned");
    }
}
