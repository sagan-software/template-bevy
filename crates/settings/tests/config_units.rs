//! Unit-bearing configuration at the public startup boundary.

use game_settings::Config;
use std::time::Duration;

/// Parses explicit units and emits canonical text without unit-suffixed keys.
#[test]
fn configuration_uses_units_in_values() {
    let config = Config::from_toml_str("tick_rate = '60 Hz'\nreplication_interval = '100 ms'\n")
        .expect("unit-bearing configuration must parse");
    assert_eq!(config.tick_rate().hertz(), 60);
    assert_eq!(
        config.replication_interval().duration(),
        Duration::from_millis(100)
    );
    assert_eq!(
        toml::to_string(&config).expect("configuration must serialize"),
        "tick_rate = \"60 Hz\"\nreplication_interval = \"100 ms\"\n"
    );
}

/// Rejects missing units, unknown units, zero values, excessive values and old
/// keys.
#[test]
fn configuration_rejects_invalid_units_and_bounds() {
    for source in [
        "tick_rate = 60",
        "tick_rate = '60'",
        "tick_rate = '60 ms'",
        "tick_rate = '0 Hz'",
        "tick_rate = '1001 Hz'",
        "tick_rate = '-1 Hz'",
        "replication_interval = 100",
        "replication_interval = '0 ms'",
        "replication_interval = '61 s'",
        "replication_interval = '1 Hz'",
        "tick_rate_hz = 60",
        "replication_interval_ms = 100",
    ] {
        assert!(Config::from_toml_str(source).is_err(), "accepted {source}");
    }
}

/// Keeps defaults and accepted duration alternatives consistent.
#[test]
fn configuration_defaults_and_duration_alternatives() {
    assert_eq!(
        Config::from_toml_str("").expect("defaults must parse"),
        Config::default()
    );
    for value in ["1 s", "1000 ms", "1 second"] {
        let source = format!("tick_rate = '1 Hz'\nreplication_interval = '{value}'");
        let config = Config::from_toml_str(&source).expect("duration alternative must parse");
        assert_eq!(config.tick_rate().period(), Duration::from_secs(1));
        assert_eq!(
            config.replication_interval().duration(),
            Duration::from_secs(1)
        );
    }
}
