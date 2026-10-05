//! Accepted input grammar, canonical output, and independent profile restrictions.

use game_settings::ReplicationInterval;
use game_settings::TickRate;
use game_settings::UnitParseError;

/// Includes both frequency bounds and normalizes leading zeroes.
#[test]
fn frequency_profile_accepts_bounds_and_normalizes_digits() {
    for (text, hertz) in [("1 Hz", 1), ("1000 Hz", 1000), ("0060 Hz", 60)] {
        let rate: TickRate = text.parse().expect("valid rate must parse");
        assert_eq!(rate.hertz(), hertz);
        assert_eq!(
            toml::Value::try_from(rate).expect("rate must serialize"),
            toml::Value::String(format!("{hertz} Hz"))
        );
    }
}

/// Rejects near misses before validating the numeric frequency.
#[test]
fn frequency_profile_rejects_lexical_near_misses() {
    for text in [
        " Hz",
        "60Hz",
        "60 hz",
        "60 HZ",
        "60  Hz",
        " 60 Hz",
        "60 Hz ",
        "+60 Hz",
        "-60 Hz",
        "60.0 Hz",
        "６０ Hz",
        "4294967296 Hz",
    ] {
        assert!(
            matches!(text.parse::<TickRate>(), Err(UnitParseError::TickSyntax)),
            "accepted {text}"
        );
    }
}

/// Tests both duration bounds, equivalent units, and exact canonical text.
#[test]
fn duration_profile_accepts_bounds_and_normalizes_units() {
    for (text, milliseconds) in [
        ("1 ms", 1),
        ("60 s", 60_000),
        ("0.1 seconds", 100),
        ("1 second", 1000),
        ("1 s", 1000),
        ("1000 ms", 1000),
    ] {
        let interval: ReplicationInterval = text.parse().expect("valid interval must parse");
        assert_eq!(interval.milliseconds(), milliseconds);
        assert_eq!(
            toml::Value::try_from(interval).expect("interval must serialize"),
            toml::Value::String(format!("{milliseconds} ms"))
        );
    }
}

/// Distinguishes parser failure, precision loss, overflow, and validated
/// bounds.
#[test]
fn duration_profile_preserves_distinct_rejection_categories() {
    assert!(matches!(
        "1 Hz".parse::<ReplicationInterval>(),
        Err(UnitParseError::Duration(_))
    ));
    assert!(matches!(
        "0.5 ms".parse::<ReplicationInterval>(),
        Err(UnitParseError::Precision)
    ));
    assert!(matches!(
        "18446744073709551615 s".parse::<ReplicationInterval>(),
        Err(UnitParseError::Overflow)
    ));
    for text in ["0 ms", "60001 ms"] {
        assert!(matches!(
            text.parse::<ReplicationInterval>(),
            Err(UnitParseError::ReplicationInterval(_))
        ));
    }
    for text in ["0 Hz", "1001 Hz"] {
        assert!(matches!(
            text.parse::<TickRate>(),
            Err(UnitParseError::TickRate(_))
        ));
    }
}

/// Applies the same bounds to standard numeric conversion as text ingress.
#[test]
fn numeric_conversions_preserve_frequency_and_duration_bounds() {
    assert_eq!(
        TickRate::try_from(60).expect("valid hertz"),
        TickRate::default()
    );
    assert_eq!(
        ReplicationInterval::try_from(100).expect("valid milliseconds"),
        ReplicationInterval::default()
    );
    for hertz in [0, 1001] {
        assert!(TickRate::try_from(hertz).is_err());
    }
    for milliseconds in [0, 60001] {
        assert!(ReplicationInterval::try_from(milliseconds).is_err());
    }
}
