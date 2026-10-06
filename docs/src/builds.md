# Build caches

Build settings and sources were inspected on 2026-10-05.
The template keeps development code at optimization level 1 and registry
dependencies at level 3, following [Bevy's setup guidance](https://bevy.org/learn/quick-start/getting-started/setup/).
This preserves optimized Bevy code during development without using the release
profile for every edit. Release-only thin LTO and one codegen unit remain unchanged.

## Existing settings and changes

The template already separates settings, physics, assets, game, and application
crates. Cargo recompiles affected crates and their consumers; stable boundaries
reduce recompilation when unrelated crates remain unchanged. More crates alone
do not shorten dependency compilation. Use timings before splitting another crate.

Development retains incremental compilation for workspace and path crates.
[Cargo applies incremental compilation only to those crates](https://doc.rust-lang.org/cargo/reference/profiles.html#incremental).
The wrapper uses [sccache](https://github.com/mozilla/sccache/blob/main/docs/Rust.md)
for non-incremental registry libraries. It does not disable incremental compilation
across the workspace.

The compiler launcher removes only `CARGO_TARGET_DIR` and
`CARGO_BUILD_BUILD_DIR` before invoking sccache, because sccache hashes Cargo
environment variables. Cargo itself retains both settings. Those directory variables
are unavailable to compile-time macros; package metadata and other environment
values remain available. Compiler invocations that link executables, dynamic libraries
or procedural macros remain outside sccache's Rust cache.

The wrapper uses [ccache](https://ccache.dev/manual/latest.html) in `CC` and `CXX`
for native C/C++ dependencies. It preserves the selected compiler and prevents
repeated setup from nesting ccache. It does not add a second CMake compiler launcher.

Rust 1.99 uses LLD by default on x86-64 Linux, as introduced in
[Rust 1.90](https://blog.rust-lang.org/2025/09/18/Rust-1.90.0/).
Windows MSVC selects rust-lld explicitly; macOS keeps its system linker.
The Nix environment supplies LLD. No extra Linux linker flags invalidate the cache.
Mold remains an alternative to benchmark, not a second default linker.

Optional Bevy dynamic linking remains available with `--features dynamic_linking`.
Keep that feature consistent across workers when measuring reuse. Shipping builds
retain static linking. Nightly-only optimization flags remain outside stable builds.

Nix now maintains separate development and release dependency artifacts.
Tests and documentation use development artifacts; packages and benchmarks use
release artifacts. Profile and feature differences still require their matching
compiled artifacts, as [Crane documents](https://crane.dev/faq/constant-rebuilds.html).

## Select the cache

The cache-launcher workflow supports Unix hosts. On Windows, use ordinary Cargo;
the wrapper rejects this workflow before changing the environment. Native Windows
compiler-cache launchers remain unverified.

The Nix shell supplies Python, sccache and ccache and applies the cache environment.
For ordinary Cargo, install Python 3, sccache, ccache, Git and the pinned Rust toolchain.
Python 3.10 or later must be available as `python`. Cargo resolves the workspace
root with `locate-project`; nested generated projects keep their own workspace.

```sh
nix develop
python scripts/cargo-fast.py --print-env
python scripts/cargo-fast.py build --locked -p template-bevy --timings
python scripts/cargo-fast.py test --locked -p game-settings
```

For generated projects, replace `template-bevy` with their application package name.
The wrapper forwards Cargo arguments directly and propagates its exit status.
An invocation without arguments runs `cargo build --locked`.

The cache defaults to `$XDG_CACHE_HOME/bevy-build`, or `~/.cache/bevy-build`.
`BEVY_BUILD_CACHE` selects another absolute cache root. Repository identity comes
from the canonical Git common directory, so linked worktrees share that identity.
Compiler identity includes `rustc -vV`, Cargo's version, and the native
compilers' resolved paths, version output and executable metadata. Separate clones and
compiler versions receive separate Cargo directories. Their compiler caches can
still reuse entries when the compiler cache's own keys match.

The default lane isolates final `CARGO_TARGET_DIR` outputs per canonical workspace
path. Eligible `build`, `check`, `run` and `test` commands share a repository-scoped
`CARGO_BUILD_BUILD_DIR`. Each worktree and lane gets a distinct real
`RUSTC_WORKSPACE_WRAPPER` executable. Cargo includes that wrapper in workspace
artifact hashes, as [Cargo documents](https://doc.rust-lang.org/cargo/reference/config.html#buildrustc-workspace-wrapper).
Registry dependencies and procedural macros reuse their existing compiled artifacts.

The helper installs namespace launchers atomically and retains their timestamps
when bytes are unchanged. The outer compiler-cache launcher has one stable path
across worktrees. Both Cargo directory settings override ambient host settings.
No global Cargo configuration changes are required.

The helper reads Cargo metadata format 1 with the requested feature switches. Sharing requires every
mutable path package to be a workspace member. Non-member dependencies must come
from known registry, sparse-registry or Git sources under Cargo's source cache or
the immutable Nix store. Vendored sources, non-member path packages, unknown source
kinds and invalid metadata select isolated intermediates.

Generated projects omit the weak asset-MCP feature when Blender assets are omitted.
Cargo metadata can otherwise include that unused path package and force isolation.
Package IDs remain opaque;
new unused metadata fields are ignored.

Clippy replaces the workspace wrapper with its own driver. Shared intermediates
then accepted an old-mtime `compile_error!` source from another worktree in the
probe. Clippy and every xtask invocation therefore select private intermediates.
Unknown commands and configuration, manifest, target-directory or toolchain overrides
also select isolation. `--cache-mode isolated` forces that policy for custom tooling.

`--shell-env` always exports isolated intermediates because it cannot determine
which Cargo command runs later. Ordinary Cargo inside Nix remains safe; use the
Python helper for direct dependency-artifact reuse in worktree builds. The compiler
cache still applies to ordinary Cargo.

A shared final target can launch another worktree's executable under concurrency.
The probe reproduced that race even with private build directories. Do not use
`--lane shared` concurrently; that explicit lane requires serialized commands and
launches. Plain commands with namespaced shared intermediates passed A/B/A,
malformed-source and concurrent feature probes. [Cargo issue 17312](https://github.com/rust-lang/cargo/issues/17312)
records the unguarded shared-directory failure.

Nix developer shells inserted a nonexistent worktree-local `outputs/out/lib`
RUNPATH into procedural macros. Their different bytes prevented downstream Bevy
compiler-cache hits. The helper removes only exact, absent developer-shell output
library/header search paths and preserves real native search paths. Normalized
native flags and compiler commands contribute to the repository cache identity.

On Unix, the wrapper selects a cache-specific sccache socket. Its encoded path must
fit within 100 bytes; choose a shorter cache root when the wrapper rejects it.
The default compiler-cache limits are 20 GiB for sccache and 5 GiB for ccache.
`SCCACHE_CACHE_SIZE` and `CCACHE_MAXSIZE` override those limits. These limits do not
bound Cargo artifact storage.

## Concurrent workers

Before dispatching workers, warm the intended compiler, profile and feature set once.

```sh
python scripts/cargo-fast.py build --locked -p template-bevy --timings
```

New worktrees automatically select separate lanes. When multiple workers use one
worktree, give each worker its own explicit lane.

```sh
python scripts/cargo-fast.py --lane agent-physics test --locked -p game-physics --features dim2
python scripts/cargo-fast.py --lane agent-settings test --locked -p game-settings
```

Separate lanes isolate final Cargo outputs and workspace compiler namespaces.
Eligible commands retain shared dependency intermediates and compiler-cache entries.
Cargo serializes access to shared intermediates; warm workers can wait for that
build lock. This avoids duplicate dependency builds and shared final-output replacement.

Lane names allow 1–64 letters, digits, underscores or hyphens,
starting with a letter or digit. Reuse the same lane for one worker's edit/build loop.

A new lane or worktree still runs Cargo fingerprint checks, build scripts and linking.
It reuses compiled registry libraries and procedural macros directly. Local crates
build with that worktree's namespace; sccache remains available for cacheable misses.
Changed dependencies, features, profiles, compiler versions and Rust flags can miss
cache entries. The first build of a new configuration remains expensive.

The wrapper rejects `clean`. Never clean another worker's cache. For a deliberate
isolated clean, use ordinary Cargo with explicitly selected private directories.
Coverage and custom tooling must use `--cache-mode isolated` and unique lanes.
Instrumented artifacts and cleanup belong to that worker.

Generated fixtures and coverage reports remain under
repository-local `target/generated` and `target/llvm-cov`; their build artifacts use
the selected Cargo environment. Build output location does not change report paths.

## Other workflows

`nix run .#dylint` clears the Cargo build-directory override and compiler wrappers
because Dylint selects its own compiler and wrapper. Before ordinary Cargo Dylint,
clear those variables as well.

```sh
unset CARGO_BUILD_BUILD_DIR RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER
cargo dylint --all --workspace -- --all-targets
```

Set `BEVY_BUILD_CACHE_DISABLE=1` before entering Nix to retain explicitly managed
Cargo directories for profiling or isolated validation. The explicit Python wrapper
still selects its own cache environment when called.

Inspect reuse with Cargo's `--timings` and `--message-format=json` output, then
inspect compiler-cache hit counters.

```sh
sccache --show-stats
ccache --show-stats
python -m unittest discover -s scripts -p 'test_*.py'
```

The automated tests use their own compiler fixtures, including inside Nix.
The suite checks forwarding and rejection separately from real build timings.
The tests cover requested feature closure and isolation of optional path packages.
They also cover environment selection, worktrees, clone/compiler
separation, lanes, invalid input, forwarding, repeated setup and generated projects.
Actual cold, cached-worktree, edit and parallel-agent measurements are recorded
separately from those command-line tests.

## Measured worktree builds

On 2026-10-05, two agents read AGENTS.md and tested fresh Git worktrees with
Rust/Cargo 1.99, matching dependency flags, and a previously compiled dependency
depot. Bevy dependencies remained at optimization level 3; workspace crates
remained at level 1. The first empty-cache application build took 324 seconds.
The shared depot was seeded from a completed matching application build;
these worktree measurements exclude that one-time dependency compilation.

Agent A's fresh build took 35.63 seconds, including helper overhead; Cargo reported
33.24 seconds. Agent B's concurrent fresh build took 65.56 seconds. Each reused
545 registry artifacts and rebuilt four workspace crates. Agent B's unchanged
repeat took 2.00 seconds, and its Cargo CLI launch took 1.60 seconds.

Agent A's unchanged repeat took 39.52 seconds while waiting for package/build locks;
all 549 artifacts were fresh. Its temporary app-comment edit took 7.48 seconds,
with only the application crate rebuilt. The edit was restored afterward. These
results separate dependency reuse, local compilation, and lock contention.

Compiler caches alone were insufficient in the earlier experiment: isolated Cargo
intermediates changed procedural-macro RUNPATH bytes and caused expensive Bevy
cache misses. Those unfinished agent runs exceeded 21 minutes and were cancelled.
The shared-dependency mode reused the macro artifacts themselves. The host also
ran unrelated Rust builds, so absolute timings are observations, not guarantees
for other machines or dependency configurations.

The safety probes covered A/B/A output, different feature sets, concurrent launches,
a fresh worktree with malformed backdated source, and Clippy/xtask isolation.
Stable Cargo checks source freshness by modification time. Manually backdating an
edit inside an already-built worktree can still hide that edit from Cargo; this
workflow does not claim checksum-based freshness. Use normal Git checkouts and edits.
