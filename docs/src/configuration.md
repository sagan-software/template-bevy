# Configuration

Run `cargo run -p template-bevy -- --config game.toml` with an explicit file:

```toml
tick_rate = "60 Hz"
replication_interval = "100 ms"
```

Without `--config`, startup uses built-in defaults without accessing the filesystem.
Omitted fields use those defaults. Unknown fields and numeric values are rejected.
The former `tick_rate_hz` and `replication_interval_ms` keys are rejected.
An explicit file must exist and parse before the game starts.
`ConfigError` preserves the selected path and underlying I/O or TOML error.

`TickRate` accepts ASCII whole digits, one space, and case-sensitive `Hz`.
The inclusive range is 1–1000 Hz. Signs, decimals, missing units, extra spaces,
and other casing are rejected. Leading zeroes are accepted and removed on output.
Serialization emits `"60 Hz"` for the default rate.
One fixed tick lasts `1 / frequency` seconds; the stored frequency is in hertz.

`ReplicationInterval` stores `Duration` and accepts the
[humantime 2.4 duration grammar](https://docs.rs/humantime/2.4.0/humantime/fn.parse_duration.html).
Accepted values must be positive, at most 60 seconds, and an exact number of
milliseconds. For example, `"1 s"`, `"1 second"`, and `"1000 ms"` are equivalent.
Fractional milliseconds such as `"0.5 ms"` are rejected.

Serialization emits whole milliseconds followed by one space and `ms`.
The examples above all emit `"1000 ms"`; the default emits `"100 ms"`.
Parsing therefore normalizes accepted duration spellings.

Both domain types validate direct numeric construction and `FromStr` input.
Serde and BRP reflection use the same string parser and canonical serialization.
Tick duration derives from the frequency; it is not stored independently.

The native inspector requires the `mcp` Cargo feature and the `--editor` flag.
Editor activation is separate from timing configuration.
