//! Screenroom desktop entry point.
//!
//! Everything of substance lives in the library target so it can be tested
//! without opening a window; see `lib.rs`.

fn main() -> iced::Result {
    screenroom_desktop::app::run()
}
