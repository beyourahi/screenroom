//! Screenroom desktop client (Windows, Linux, macOS): iced + wgpu + libmpv.
//!
//! The crate is split so the parts that carry rules can be tested without a
//! window:
//!
//! * [`playback`] decides when progress is written; the rules live in
//!   `screenroom-core`,
//! * [`artwork`] extracts one poster per work, off the UI thread and bounded,
//! * [`picker`] is the platform's own folder chooser,
//! * [`video_surface`] bridges libmpv frames to wgpu,
//! * [`quit`] makes the menu bar's Quit take the same path as closing the window,
//! * [`app`] is the iced interface and owns nothing that a test needs to trust.

pub mod app;
pub mod artwork;
pub mod design;
pub mod picker;
pub mod playback;
pub mod quit;
pub mod video_surface;
