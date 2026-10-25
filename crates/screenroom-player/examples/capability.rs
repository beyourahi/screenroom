//! Prints this device's libmpv capability and preferred render bridge.
//!
//! Run with `cargo run -p screenroom-player --example capability`.

fn main() {
    let capability = screenroom_player::capability();
    println!("available:     {}", capability.available);
    println!("reason:        {}", capability.reason);
    println!(
        "library path:  {}",
        capability.library_path.as_deref().unwrap_or("-")
    );
    println!(
        "api version:   {}",
        capability.api_version.as_deref().unwrap_or("-")
    );
    println!("bridge:        {:?}", screenroom_player::preferred_bridge());
}
