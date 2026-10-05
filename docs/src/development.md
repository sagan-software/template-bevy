# Development

Ordinary Cargo works independently of Nix. Install the pinned Rust toolchain with
Rustup and follow [Bevy's platform setup](https://bevy.org/learn/quick-start/getting-started/setup/)
for native libraries. Linux needs graphics, audio, input, and window-system libraries.
The Nix development shell supplies those libraries and the project tools.

```sh
cargo run -p template-bevy
cargo run -p template-bevy --features mcp -- --editor
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo xtask imports
cargo xtask setup
```

Use the generated package name in place of `template-bevy`.
`cargo xtask setup` creates the relative `CLAUDE.md → AGENTS.md` link after generation.
Cargo-generate does not copy symlinks. Setup preserves an existing regular `CLAUDE.md`
and repairs dangling symlinks. Windows requires Developer Mode or symlink permission.
Repository-local skills live in `.agents/skills`; setup does not invent skill links.

## Reproducible tools

```sh
nix develop
nix run
nix run .#editor
nix run .#check
nix flake check
nix build
```

`nix run .#check` runs treefmt, portable checks, and public Dylints.
Rustfmt formats Rust; nixfmt formats Nix. The import check requires one item per
`use` statement, including nested imports, without unstable formatting options.
Individual commands include `.#test`, `.#clippy`, `.#features`, `.#bench`,
`.#book`, `.#coverage`, and `.#generate-matrix`.

## Dylints

The public [Dylints quick start](https://github.com/sagan-software/dylints)
uses Cargo workspace metadata and the `correctness`, `perf`, and `suspicious` groups.
Install Cargo Dylint, its linker, and the compiler selected by the lint library:

```sh
cargo install cargo-dylint dylint-link --version 6.0.3 --locked
rustup toolchain install nightly-2026-07-15 --component rustc-dev,llvm-tools-preview
```

Then run:

```sh
cargo dylint --all --workspace -- --all-targets
```

With Nix, run `nix run .#dylint`. The packaged runner includes its compiler and
lint libraries; it requires no local Dylints checkout or private filesystem path.
The Nix lint check uses a writable cache under `$TMPDIR/sagan-lints`.

Cargo metadata and `flake.nix` pin the same public revision. Update both pins and
`flake.lock` together. The packaged Nix runner treats warnings as errors.
The Cargo quick start uses each lint library's default severity.

If `CARGO_BUILD_BUILD_DIR` is set, clear it for Cargo Dylint 6.0.3 as described in
[compatibility boundaries](dependencies.md#compatibility-boundaries).
The full Bevy policy group forbids schedules this starter deliberately uses,
so it is excluded from the default groups.

The Dylints compiler is pinned separately from application Rust.
The bundle supports x86_64 Linux, aarch64 Linux, and aarch64 macOS.
Intel macOS reports unsupported status and omits isolated Dylints checks.
An onboarding fixture verifies that an owned read-only parameter fails and its
borrowed replacement passes. Platform support does not establish runtime verification.

## Lint policy

Cargo manifests separate enforced checks, justified allowances, and unavailable
nightly-only checks. Rustdoc retains its warnings without a blanket allowance.
Clippy groups use priority -1; individual rules override them at priority 1 or 2.
Authored private documentation is denied; Clap-generated items retain derive allowances.

Rust allowances cover linker notices, feature-selected dependency use, and Bevy's
chainable return values. Clippy allowances cover unpublished package metadata,
third-party duplicate versions, local futures, ECS guards and parameters, explicit
crate visibility, and framework-owned values. The full restriction group contains
conflicting policies; selected restriction checks remain enabled individually.
Nightly-only rustc rules remain excluded until the stable compiler supports them.
Keep each allowance's reason beside its manifest entry.

## Profiles and profiling

Profiles live in `.cargo/config.toml`, as supported by
[Cargo's profile configuration](https://doc.rust-lang.org/cargo/reference/config.html#profile).
Development uses optimization level 1, with dependencies at level 3.
Release uses one codegen unit and thin LTO. The `wasm-release` profile uses size
optimization. These choices follow
[Bevy's setup guidance](https://bevy.org/learn/quick-start/getting-started/setup/).
Windows uses `rust-lld`; macOS retains its system linker.

Dynamic linking and `dev`, `trace_chrome`, and `trace_tracy` remain optional.
`cargo xtask features` checks each optional mode independently.
It also runs game tests for local 2D, local 3D, networked 3D, and both dimensions.
These tests cover dimension-specific code that a combined feature build leaves inactive.

The Nix shell includes cargo-flamegraph, Samply, Hyperfine, and Linux perf.
Criterion is a Cargo development dependency.
Criterion benchmarks cover deterministic movement and warmed ECS schedules.
Validation compiles benchmarks without imposing timing thresholds on shared hardware.

```sh
cargo bench --workspace
cargo flamegraph --bin template-bevy
samply record cargo run -p template-bevy
cargo build --timings
cargo tree -e features
```

Profiler availability does not guarantee permission to sample the host.
Inspect measurements before changing performance settings.

## Browser

Install Trunk and the `wasm32-unknown-unknown` target, or use `nix develop`.
When generating, select browser support. Then run:

```sh
trunk serve
trunk build --cargo-profile wasm-release
```

Open `http://127.0.0.1:8080`. The canvas accepts WASD, arrows, and Space for boost.
Trunk writes deployable files to `dist/`; `public_url = "./"` supports subdirectories.
Browser builds use WebGL2 and do not start native inspector listeners or server threads.
Trunk's build runs Binaryen optimization. Browser profiling and native GPU tooling
have different capabilities; the browser does not include native Hanabi particles.

## Coverage and documentation

Install cargo-llvm-cov and mdBook, or use `nix develop`.
`cargo xtask coverage` runs instrumented tests once and emits HTML, LCOV, and JSON
under `target/llvm-cov/`. The total line floor is 50%; changed deterministic logic
should reach 100%. Unit tests run under coverage as well as ordinary tests.
The aggregate includes inline test functions; inspect production regions separately
before making production-coverage claims. Doctests run separately on stable Rust.

```sh
cargo xtask book
mdbook serve docs
cargo doc --workspace --no-deps --document-private-items
```

Technical documentation belongs in `docs/src/`, with navigation in `SUMMARY.md`.
Keep the README focused on generation, launch, controls, and actual game media.
The [review checklist](review.md) records the source feedback and verification boundary.

The game uses Bevy's logging plugin without a direct log dependency. Development
launches default to information logging; release launches default to warning
logging. `RUST_LOG` can override the runtime filter.

The optional Bevy CLI compiler cannot check this project's minimum Rust version.
See [dependency compatibility](dependencies.md#compatibility-boundaries).

Trunk 0.21.14 parses `NO_COLOR` as a Boolean. On Unix, use
`NO_COLOR=true trunk build` or `NO_COLOR=true trunk serve` when the inherited
value is `1`. The generation matrix sets this value for Trunk automatically.
The HTML selects the `wasm-release` Cargo profile and Binaryen size optimization.
[Trunk's Rust asset pipeline](https://github.com/trunk-rs/trunk/blob/v0.21.14/src/pipelines/rust/mod.rs)
defines these profile and optimization attributes.
