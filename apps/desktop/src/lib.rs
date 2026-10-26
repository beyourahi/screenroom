//! Screenroom desktop client (Windows, Linux, macOS): iced + wgpu + libmpv.
//!
//! The crate is split so the parts that carry rules can be tested without a
//! window:
//!
//! * [`playback`] decides when progress is written; the rules live in
//!   `screenroom-core`,
//! * [`video_surface`] bridges libmpv frames to wgpu,
//! * [`app`] is the iced interface and owns nothing that a test needs to trust.

pub mod app;
pub mod playback;
pub mod video_surface;
