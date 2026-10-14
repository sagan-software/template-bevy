# Dependency compatibility

Verified against published package manifests on 2026-10-14. The selected engine
is [Bevy 0.20.0](https://bevy.org/news/bevy-0-20/). Rust remains pinned to 1.99.0.
Root and generated manifests use the same versions.

Compatible releases selected:

- [Rapier 0.37.0](https://docs.rs/crate/bevy_rapier2d/0.37.0/source/Cargo.toml) for 2D and 3D physics.
- [enhanced_input 0.27.0](https://docs.rs/crate/bevy_enhanced_input/0.27.0/source/Cargo.toml).
- [BRP extras 0.23.0](https://docs.rs/crate/bevy_brp_extras/0.23.0/source/Cargo.toml).

These published releases still depend on Bevy 0.19 and cannot enter the 0.20 graph:

- [Hanabi 0.19.0](https://docs.rs/crate/bevy_hanabi/0.19.0/source/Cargo.toml).
- [inspector-egui 0.37.0](https://docs.rs/crate/bevy-inspector-egui/0.37.0/source/Cargo.toml).
- [Lightyear 0.30.1](https://docs.rs/crate/lightyear/0.30.1/source/Cargo.toml).
- [Skein 0.6.0](https://docs.rs/crate/bevy_skein/0.6.0/source/Cargo.toml).
- [Avian 0.7.0](https://docs.rs/crate/avian2d/0.7.0/source/Cargo.toml).

Rapier is the existing template's physics engine; Avian is not added as a second
engine. Networking and Skein generation choices reject true with an explicit
compatibility error. Their existing source is retained behind feature and
`bevy_020_lightyear` or `bevy_020_skein` cfg gates. Inspector source uses
`bevy_020_inspector`. These cfg gates are not enabled. Restore compatible
dependencies and migrate APIs before enabling any gate.

The public `networked` and `skein` Cargo feature names remain compatibility
markers for existing configurations. They do not activate those integrations.
`--editor` reports the unavailable inspector. BRP remains available through MCP.
Hanabi is omitted from the compiled dependency graph. Native particles,
network authority receipts, and Skein asset ingestion are therefore unavailable
in this branch, and their old runtime claims do not apply.

Bevy 0.20 replaces the old material shader imports with WESL. The embedded
fragments and shader validation tests are migrated to that interface. See
[shaders](shaders.md) for bindings and runtime validation requirements.

The MCP CLI remains pinned independently of the engine. Protocol compatibility
must be verified against the running Bevy 0.20 BRP endpoint before use. Packaging
scaffolds follow Bevy's mobile examples; Linux cannot verify the Xcode target.
