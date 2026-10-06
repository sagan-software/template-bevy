# Dependency compatibility

Verified on 2026-10-05. Template pins are reproducible snapshots; verify current
stable releases and target compatibility before generating or updating a project.

Rust 1.99.0 was published on 2026-10-01 in the
[official stable channel manifest](https://static.rust-lang.org/dist/channel-rust-stable.toml).
The toolchain, Cargo minimum Rust version, and Clippy minimum Rust version agree.
Bevy uses the [0.19.1 release](https://github.com/bevyengine/bevy/releases/tag/v0.19.1).
Cargo manifests and Cargo/Nix lockfiles are updated together.

## Selected integrations

The selected plugin manifests accept Bevy 0.19. The package sources below record
these dependency requirements; a shared version range does not prove runtime behavior.

- [bevy_rapier2d 0.36.0 manifest](https://docs.rs/crate/bevy_rapier2d/0.36.0/source/Cargo.toml).
- [bevy_rapier3d 0.36.0 manifest](https://docs.rs/crate/bevy_rapier3d/0.36.0/source/Cargo.toml).
- [bevy_brp_extras 0.22.8 manifest](https://docs.rs/crate/bevy_brp_extras/0.22.8/source/Cargo.toml).
- [bevy_hanabi 0.19.0 manifest](https://docs.rs/crate/bevy_hanabi/0.19.0/source/Cargo.toml).
- [bevy-inspector-egui 0.37.0 manifest](https://docs.rs/crate/bevy-inspector-egui/0.37.0/source/Cargo.toml).
- [bevy_enhanced_input 0.26.0 manifest](https://docs.rs/crate/bevy_enhanced_input/0.26.0/source/Cargo.toml).
- [lightyear 0.30.1 manifest](https://docs.rs/crate/lightyear/0.30.1/source/Cargo.toml).
- [bevy_skein 0.6.0 manifest](https://docs.rs/crate/bevy_skein/0.6.0/source/Cargo.toml).
- [bevy_brp_mcp 0.22.8 manifest](https://docs.rs/crate/bevy_brp_mcp/0.22.8/source/Cargo.toml).

## Compatibility boundaries

Rapier's enhanced determinism feature enables scalar math. Bevy reflection and
serialization for the selected glam version fail to compile with that combination.
The template omits enhanced determinism and makes no cross-platform bit-identical
simulation guarantee. Track the
[Rapier compatibility issue](https://github.com/dimforge/bevy_rapier/issues/697)
before enabling it.

Hanabi particles run on native targets. The browser uses WebGL2 and omits Hanabi,
whose browser support requires WebGPU. Native inspector and MCP dependencies are
also excluded from browser builds. Browser authority updates on the browser thread
instead of starting an operating-system thread.

The optional Bevy CLI snapshot pins Rust nightly 2026-04-16, which reports Rust
1.97. The project requires Rust 1.99. A real check with that compiler rejects the
project's minimum Rust version. Bevy Lint therefore remains outside default checks;
`nix run .#bevy` exposes the optional CLI without claiming lint compatibility.
Do not bypass the minimum version to describe that gate as passing.

Dylints uses Rust nightly 2026-07-15 separately from application Rust. The public
bundle and Cargo metadata pin revision
`9bc21efeccdd1236e3e64cf7bb607823c60d4600`.
Cargo Dylint 6.0.3 does not find built libraries when an external Cargo build
directory relocates them. Clear `CARGO_BUILD_BUILD_DIR` for the Cargo quick start,
or use the packaged Nix runner. This is a tool limitation, not a project setting.

Conditional item groups use the standard library's
[`cfg_select!` macro](https://doc.rust-lang.org/std/macro.cfg_select.html), stable
since Rust 1.95. It preserves crate-root plugin exports without another dependency.

Bevy Enhanced Input 0.26.0 emits transient `MovementAxis2D` conversion warnings
for its initial Boolean snapshot in native networking mode. Runtime checks must
still verify vector input, authority receipts, action release, and interpolation.
The template does not suppress these upstream warnings.

Shader support uses Bevy's built-in Material2d and Material APIs, with WGSL
embedded through its asset loader. Bevy 0.19.1 and Rust 1.99.0 remained the latest
stable releases when checked on 2026-10-05 against their official
[Bevy release](https://github.com/bevyengine/bevy/releases/tag/v0.19.1) and
[Rust release](https://github.com/rust-lang/rust/releases/tag/1.99.0).
The existing lockfile selects wgpu/Naga 29.0.4 and naga_oil 0.22.0.
See [shaders](shaders.md) for source interfaces and browser restrictions.
