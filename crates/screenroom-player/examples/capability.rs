//! Prints this device's libmpv capability and preferred render bridge.
//!
//! ```bash
//! cargo run -p screenroom-player --example capability
//! cargo run -p screenroom-player --example capability -- dist/Screenroom.app/Contents/Frameworks/libmpv.2.dylib
//! ```
//!
//! With no argument it reports whatever this machine would use normally, which is
//! the system copy. With an explicit path it reports that exact build instead —
//! that is the only way to check the copy a packaged application carries, because
//! the normal search prefers the system paths on a development machine.

fn main() {
    let error = match std::env::args().nth(1) {
        None => {
            let capability = screenroom_player::capability();
            report(&capability);
            None
        }
        Some(path) => {
            let capability = screenroom_player::capability_at(&path);
            println!("requested:     {path}");
            report(&capability);
            (!capability.available).then(|| capability.reason.clone())
        }
    };
    // A check that names a path is asking whether *that* library works, so it must
    // fail loudly when it does not, rather than printing a reason and exiting zero.
    if let Some(reason) = error {
        eprintln!("the requested libmpv is not usable: {reason}");
        std::process::exit(1);
    }
}

fn report(capability: &screenroom_player::PlayerCapability) {
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
