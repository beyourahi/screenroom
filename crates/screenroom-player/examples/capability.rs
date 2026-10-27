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
    let argument = std::env::args().nth(1);
    if argument.as_deref() == Some("--explain") {
        explain();
        return;
    }
    let error = match argument {
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

/// Prints every path `load` would try, in order, plus how the order was chosen.
///
/// This exists for the Windows machine, which cannot be inspected from the Mac that
/// develops the code: a bare "no libmpv shared library found" is a blank refusal, and
/// the interesting question there is whether the install's own copy was reached
/// before the platform search — which is a security property, not just a preference.
fn explain() {
    println!(
        "bundle layout:   {:?}",
        screenroom_player::BundleLayout::host()
    );
    println!(
        "platform search consults the working directory: {}",
        screenroom_player::system_search_consults_working_directory()
    );
    println!();
    println!("search order (first match wins):");
    for (index, path) in screenroom_player::libmpv_search_plan().iter().enumerate() {
        let exists = if path.exists() { "present" } else { "absent" };
        println!("  {:>2}. {exists:<7} {}", index + 1, path.display());
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
