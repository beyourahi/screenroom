//! Temporary diagnostic, not committed.

use screenroom_core::Library;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database = std::env::args().nth(1).expect("database");
    let library = Library::open(&database)?;
    let profile = library.profiles()?.remove(0);
    println!("profile: {} {}", profile.id, profile.name);
    let watching = library.continue_watching(&profile.id, 20)?;
    println!("continue watching: {}", watching.len());
    for item in &watching {
        println!(
            "  {} · {} pos={} dur={}",
            item.title,
            item.episode_label(),
            item.position_ms,
            item.duration_ms
        );
    }
    let recommendations = library.recommendations(&profile.id, 20)?;
    println!("recommendations: {}", recommendations.len());
    for recommendation in &recommendations {
        println!(
            "  {} · {} — {}",
            recommendation.item.title,
            recommendation.item.episode_label(),
            recommendation.reason
        );
    }
    Ok(())
}
