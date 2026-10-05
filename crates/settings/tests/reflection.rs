//! Reflected configuration preserves unit-bearing serialization and validation.

use bevy::reflect::ReflectDeserialize;
use bevy::reflect::ReflectSerialize;
use bevy::reflect::TypeRegistry;
use bevy::reflect::serde::TypedReflectSerializer;
use game_settings::ReplicationInterval;
use game_settings::TickRate;
use serde::de::value::Error;
use serde::de::value::StrDeserializer;
use std::any::TypeId;

/// Uses the same validated frequency boundary through Bevy's remote reflection.
#[test]
fn reflected_tick_rate_serializes_units_and_rejects_zero() {
    let mut registry = TypeRegistry::default();
    registry.register::<TickRate>();
    let registration = registry
        .get(TypeId::of::<TickRate>())
        .expect("frequency must be registered");
    assert!(registration.data::<ReflectSerialize>().is_some());
    let deserializer = registration
        .data::<ReflectDeserialize>()
        .expect("frequency must support reflected deserialization");
    assert!(
        deserializer
            .deserialize(StrDeserializer::<Error>::new("0 Hz"))
            .is_err()
    );
    let value = deserializer
        .deserialize(StrDeserializer::<Error>::new("60 Hz"))
        .expect("valid frequency must deserialize");
    assert_eq!(
        value
            .downcast_ref::<TickRate>()
            .expect("frequency retains its domain type")
            .hertz(),
        60
    );
    let serialized = toml::Value::try_from(TypedReflectSerializer::new(value.as_ref(), &registry))
        .expect("reflection must serialize frequency");
    assert_eq!(serialized, toml::Value::String("60 Hz".into()));
}

/// Uses the same precision restriction through reflected duration ingress.
#[test]
fn reflected_interval_serializes_units_and_rejects_submillisecond_values() {
    let mut registry = TypeRegistry::default();
    registry.register::<ReplicationInterval>();
    let registration = registry
        .get(TypeId::of::<ReplicationInterval>())
        .expect("interval must be registered");
    assert!(registration.data::<ReflectSerialize>().is_some());
    let deserializer = registration
        .data::<ReflectDeserialize>()
        .expect("interval must support reflected deserialization");
    assert!(
        deserializer
            .deserialize(StrDeserializer::<Error>::new("0.5 ms"))
            .is_err()
    );
    let value = deserializer
        .deserialize(StrDeserializer::<Error>::new("1 second"))
        .expect("valid interval must deserialize");
    assert_eq!(
        value
            .downcast_ref::<ReplicationInterval>()
            .expect("interval retains its domain type")
            .milliseconds(),
        1_000
    );
    let serialized = toml::Value::try_from(TypedReflectSerializer::new(value.as_ref(), &registry))
        .expect("reflection must serialize interval");
    assert_eq!(serialized, toml::Value::String("1000 ms".into()));
}
