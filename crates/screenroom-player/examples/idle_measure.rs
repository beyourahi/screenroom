//! Samples CPU and RSS while one offscreen Player has no media loaded.

use std::process::Command;
use std::time::{Duration, Instant};

use screenroom_player::Player;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let seconds: u64 = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "30".to_string())
        .parse()?;
    if seconds < 30 {
        return Err("idle sample must run for at least 30 seconds".into());
    }
    let player = Player::start()?;
    // Keep player construction out of the steady-state sample window.
    std::thread::sleep(Duration::from_secs(2));
    let wake_count_at_start = player.wake_notifications();
    let started = Instant::now();
    let mut samples = Vec::<(f64, u64)>::new();
    while started.elapsed() < Duration::from_secs(seconds) {
        let output = Command::new("ps")
            .args(["-o", "%cpu=,rss=", "-p", &std::process::id().to_string()])
            .output()?;
        let line = String::from_utf8(output.stdout)?;
        let mut fields = line.split_whitespace();
        if let (Some(cpu), Some(rss)) = (fields.next(), fields.next()) {
            samples.push((cpu.parse()?, rss.parse()?));
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    let cpu_mean = samples.iter().map(|(cpu, _)| cpu).sum::<f64>() / samples.len() as f64;
    let cpu_max = samples.iter().map(|(cpu, _)| *cpu).fold(0.0, f64::max);
    let rss_mean = samples.iter().map(|(_, rss)| *rss).sum::<u64>() as f64 / samples.len() as f64;
    let rss_max = samples.iter().map(|(_, rss)| *rss).max().unwrap_or(0);
    let snapshot = player.snapshot();
    let wakeups = player
        .wake_notifications()
        .saturating_sub(wake_count_at_start);
    println!(
        "seconds={},samples={},cpu_mean_percent={cpu_mean:.2},cpu_max_percent={cpu_max:.2},rss_mean_kb={rss_mean:.0},rss_max_kb={rss_max},wake_notifications={wakeups},player_state={:?},loaded={}",
        started.elapsed().as_secs(),
        samples.len(),
        snapshot.state,
        snapshot.loaded,
    );
    Ok(())
}
