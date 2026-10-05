//! Parses configuration through the public settings API before gameplay starts.
//!
//! Both fields contain explicit units. Parsing validates frequency, duration,
//! and precision before returning typed values. The frequency determines one
//! fixed-step duration; the snapshot interval retains its duration independently.
//! Run this example with `cargo run -p game-settings --example configuration`.

use game_settings::Config;

/// Prints validated timing values without accessing a configuration file.
fn main() -> Result<(), toml::de::Error> {
    let config =
        Config::from_toml_str("tick_rate = \"60 Hz\"\nreplication_interval = \"1 second\"")?;
    let hertz = config.tick_rate().hertz();
    let milliseconds = config.replication_interval().milliseconds();
    println!("tick_rate = {hertz} Hz; replication_interval = {milliseconds} ms");
    Ok(())
}
