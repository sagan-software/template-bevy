# template-bevy

A Nix-first [Bevy 0.19](https://bevy.org/news/bevy-0-19/) starter that
demonstrates a real networked 2D game loop. It combines Avian2D physics,
Lightyear prediction and interpolation, Bevy Enhanced Input, Hanabi particles,
runtime diagnostics, an optional-at-runtime world inspector, and Bevy Remote
Protocol (BRP) tooling for people and AI agents.

The example intentionally runs two separate Bevy `App`s in one process:

```text
rendered client App                    headless authority App
DefaultPlugins                        MinimalPlugins
Lightyear client                      Lightyear server
prediction + interpolation   <---->   authoritative simulation
local input bindings                  replicated action consumption
          paired in-memory Crossbeam transport
```

This is not a same-world host shortcut. Each world owns its own entities,
schedules, networking registry, and physics state. The in-memory transport keeps
the demo self-contained while exercising the same receive-side concepts used by
a socket-backed game.

## Create a project

Install [Nix](https://nixos.org/download/) with flakes enabled. The flake supplies
Rust and the native development libraries on Linux and macOS. Running the demo
requires a graphical session and a Vulkan-capable GPU on Linux.

Select **Use this template** on GitHub, or create a repository with the GitHub CLI:

```sh
gh repo create OWNER/my-game --template sagan-software/template-bevy --public --clone
cd my-game
nix run
```

GitHub copies the files and starts the new repository with its own history.
The package remains named `template-bevy`. To rename it, update `Cargo.toml`,
`Cargo.lock`, `packageName` in `flake.nix`, and `packageName` in `ai/default.nix`.
Update `template_bevy` imports and reflected type paths in `src/`, `tests/`,
`benches/`, and the documentation together.

Nix pins formatter plugin downloads in `nix/dprint-plugin-hashes.json`.
When changing plugin URLs in `dprint.json`, update their SHA-256 pins together.

The committed dependency snapshot selects Bevy 0.19.1, Lightyear 0.29.0,
and Rust 1.98.1. Review plugin compatibility before updating these pins.

## Quick start

Run all development commands through Nix. Do not invoke a host-installed Cargo,
Rust compiler, Clippy, or rustfmt.

```sh
nix run                         # start the normal demo
nix run .#editor                # start the same binary with --editor
nix run .#dev                   # opt into Bevy's development feature collection
nix run .#features              # compile each supported optional feature separately
nix run .#test                  # unit, integration, and doctests
nix run .#clippy                # Clippy across every target
nix run .#coverage              # tests plus HTML, LCOV, and JSON coverage
nix run .#bench -- --test       # discover and exercise benchmarks without timing
nix run .#check                 # the complete local validation workflow
nix flake check                 # isolated format/lint/test/doc/feature/bench/coverage/build checks
nix build                       # build the distributable application
```

The controls are:

- WASD or the arrow keys: two-axis movement;
- the left gamepad stick: analog movement with a dead zone;
- Space or the south gamepad button: movement boost.

Opposite digital directions cancel. Diagonal input is normalized so it is not
faster than axial movement, while sub-unit analog magnitudes remain available
for fine control.

## Configuration

Without `--config`, the app uses built-in defaults and does not access the
filesystem. An explicit path must exist and parse successfully:

```sh
nix run -- --config local.toml
nix run .#editor -- --config local.toml
```

Configuration is TOML with strict field names:

```toml
# Fixed simulation and Lightyear tick frequency. Default: 60 Hz.
tick_rate_hz = 60

# Delay between authoritative snapshots. Default: 100 milliseconds.
replication_interval_ms = 100
```

Omitted fields use defaults. Unknown fields, wrong types, zero values, and
values beyond the documented safety bounds are errors with the selected path
and underlying I/O or TOML cause preserved.

Editor activation is deliberately not configuration and not a Cargo feature.
The inspector code ships in every build, and only the `--editor` CLI flag adds
its plugins. This gives players, developers, and automation the same binary at
the cost of some binary size and compiled debug surface.

## What the scene teaches

The cyan circle is locally predicted. Hardware or BRP-injected keys become
typed Enhanced Input actions, Lightyear records those actions by network tick,
the client predicts their effect, and the headless server consumes the same
replicated action stream authoritatively.

The orange box is server-driven and snapshot-interpolated. The client reports
this path ready only after a receiver-created `ConfirmedHistory<Position>` has
at least two authoritative samples. A replicated marker by itself is not
treated as interpolation proof.

The arena outline, direction indicators, and Hanabi landmark are presentation-only.
The headless server contains no window, renderer, inspector, particle system,
or render diagnostics.

## Source architecture

The crate is organized by functional domain, following Bevy's project
organization guidance:

- `main`: Parse CLI, load config, add the root plugin, and run the app.
- `cli`: Clap boundary types.
- `config`: Serde configuration and validated units.
- `errors`: Source-preserving startup errors.
- `plugin`: Thin client/server composition and server-runtime ownership.
- `inputs`: Enhanced Input contexts, actions, bindings, and Lightyear adapters.
- `network`: Protocol, Crossbeam endpoints, replication lifecycle, and server thread.
- `simulation`: Shared deterministic movement, boundaries, telemetry, and rollback policy.
- `presentation`: Camera, arena drawing, body rendering, and particles.
- `inspection`: Stable BRP status, diagnostics, logs, and runtime inspector.
- `components`: Only data contracts genuinely shared by multiple domains.
- `systems`: Only cross-domain schedule labels and orchestration.

Feature-local components, resources, observers, and systems stay with their
feature plugin. `components.rs` and `systems.rs` are not global inventories.
Flat modules keep inline tests; a logical module that exceeds 1,000 total lines
is converted atomically to `src/<module>/mod.rs` plus `tests.rs`. The repository
never combines `src/<module>.rs` with child files under `src/<module>/`.

Every public and private item is documented, and non-obvious function bodies
explain scheduling, networking, physics, and ownership decisions. The source is
intended to be read as a guide and copied into new projects.

## Diagnostics and inspection

The rendered app enables:

- `LogDiagnosticsPlugin`;
- `FrameTimeDiagnosticsPlugin`;
- `EntityCountDiagnosticsPlugin`;
- `SystemInformationDiagnosticsPlugin`;
- `RenderDiagnosticsPlugin`.

System information is relatively expensive and may be unavailable with dynamic
linking. Render diagnostics require an active renderer and produce no samples
in a headless app. Release builds retain `log/release_max_level_warn`, so normal
`info` diagnostic reports can be compiled out even though the diagnostic store
and inspector still collect data.

BRP Extras listens on loopback port `15702` by default. Set
`BRP_EXTRAS_PORT` before launch to choose another loopback port. The reflected
`template_bevy::NetworkDemoStatus` resource is the stable automation contract.
It separates local semantic-action counters from authoritative server receipt
counters so a client-only keyboard observation cannot masquerade as network
input success.

## Tests and coverage

Tests follow Bevy's lowest-sufficient-layer approach: pure Rust first, then a
`World`, one system, a focused schedule, a small headless `App`, and finally a
bounded two-world integration harness. Window/GPU startup and wall-clock sleeps
do not belong in ordinary unit tests.

`nix flake check` enforces total line coverage of at least 50%. The floor is a
regression guard; deterministic production logic should remain close to 100%
where practical. Generate local reports with:

```sh
nix run .#coverage
```

The ordinary test workflow runs all module-local unit tests plus the headless
network integration test. Coverage uses Rust's `cfg(coverage)` to omit unit-test
modules from the production-code metric, because stable Rust 1.96 cannot exclude
source regions with `#[coverage(off)]`. Consequently, the coverage percentage
measures production paths reached by the integration suite; it complements,
rather than replaces, the much broader unit-test result.

The reports are written to:

- `target/llvm-cov/html/index.html` for browsing;
- `target/llvm-cov/lcov.info` for editors and CI;
- `target/llvm-cov/coverage.json` for machine inspection.

Build the immutable report package with:

```sh
nix build .#coverage-report --out-link result-coverage
```

Its three formats live under `result-coverage/`. Coverage runs the tests once,
then renders each format from the same profile data. Doctests run separately on
stable Rust because stable LLVM source coverage does not include them.

## Benchmarks and profiling

Criterion measures only deterministic work: batched movement/boundary steps
and warmed ECS schedules over representative entity counts. Flake validation
compiles benchmarks but never enforces timing thresholds on shared hardware.

```sh
nix run .#bench
nix run .#bench -- --save-baseline before
nix run .#bench -- --baseline before
```

Treat one run as a measurement, not a performance claim. Use the existing
`trace_chrome` or `trace_tracy` Cargo feature through a Nix-routed workflow for
whole-frame profiling.

Compile each supported optional mode with `nix run .#features`. This checks
`dev`, `dynamic_linking`, `trace_chrome`, and `trace_tracy` independently so an
unsupported `--all-features` combination does not masquerade as a useful build.

The development profile uses optimization level 1 for this crate and level 3
for dependencies. Dynamic linking remains opt-in, Windows uses `rust-lld`, and
macOS keeps its system linker. No nightly flags, alternate macOS linker,
Cranelift, or compiler cache is enabled without measurement.

To inspect what really compiles:

```sh
nix develop -c cargo tree -e features
nix develop -c cargo tree -d
nix develop -c cargo build --timings
```

`nix run .#features` is a repeatable compile gate. The tree and timing commands
remain inspection tools: read their output before changing dependencies,
profiles, linkers, or crate boundaries.

The project selects Bevy/Avian/Hanabi 2D features. BRP can still pull Bevy PBR
transitively through Bevy development tooling; that is not a project-selected
3D gameplay path.

## BRP and MCP smoke testing

Start the app in one retained terminal and the packaged newline-delimited MCP
server in another:

```sh
nix run
nix run .#bevy-brp-mcp
```

Agent configuration is tracked in `ai/mcp.json` and `ai/codex/config.toml`.
`nix run .#setup-ai` safely refreshes generated tool links. Do not use a launch
tool that bypasses the Nix wrapper.

For a raw stdio smoke test, send one compact JSON object per line. Initialize
the packaged v0.20.1 server, announce readiness, and inspect its live schemas:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{"roots":{"listChanged":false}},"clientInfo":{"name":"template-bevy-smoke","version":"1.0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
```

While processing every received `tools/call`, this server asks the client for
`roots/list`. Read that request, preserve its server-generated ID, and answer it
before waiting for the tool result:

```json
{"jsonrpc":"2.0","id":42,"result":{"roots":[{"uri":"file:///absolute/path/to/template-bevy","name":"template-bevy"}]}}
```

Replace `42` with the ID from the request you just received and replace the URI
with the current checkout's absolute `file://` URI. Then make narrow, sequential
calls such as:

```json
{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"brp_status","arguments":{"app_name":"template-bevy","port":15702}}}
{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"world_get_resources","arguments":{"resource":"template_bevy::NetworkDemoStatus","port":15702}}}
{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"world_query","arguments":{"data":{"components":["avian2d::physics_transform::transform::Position"],"option":["bevy_ecs::name::Name"],"has":[]},"filter":{"with":["lightyear_core::prediction::Predicted","template_bevy::components::DemoBody"],"without":[]},"strict":true,"port":15702}}}
{"jsonrpc":"2.0","id":13,"method":"tools/call","params":{"name":"brp_extras_send_keys","arguments":{"keys":["Space","ArrowRight"],"duration_ms":200,"port":15702}}}
{"jsonrpc":"2.0","id":14,"method":"tools/call","params":{"name":"world_get_resources","arguments":{"resource":"template_bevy::NetworkDemoStatus","port":15702}}}
{"jsonrpc":"2.0","id":15,"method":"tools/call","params":{"name":"world_query","arguments":{"data":{"components":["avian2d::physics_transform::transform::Position"],"option":["bevy_ecs::name::Name"],"has":[]},"filter":{"with":["lightyear_core::prediction::Predicted","template_bevy::components::DemoBody"],"without":[]},"strict":true,"port":15702}}}
{"jsonrpc":"2.0","id":16,"method":"tools/call","params":{"name":"brp_extras_get_diagnostics","arguments":{"port":15702}}}
{"jsonrpc":"2.0","id":17,"method":"tools/call","params":{"name":"brp_extras_screenshot","arguments":{"path":"/tmp/template-bevy-mcp.png","port":15702}}}
{"jsonrpc":"2.0","id":18,"method":"tools/call","params":{"name":"world_get_resources","arguments":{"resource":"template_bevy::NetworkDemoStatus","port":15702}}}
{"jsonrpc":"2.0","id":19,"method":"tools/call","params":{"name":"brp_shutdown","arguments":{"app_name":"template-bevy","port":15702}}}
```

Answer the intervening `roots/list` request after each line. A valid shutdown
reports `shutdown_method` as `clean_shutdown`, not `process_kill`. In editor
mode, require at least one Egui context alongside `editor_enabled: true` in the
status resource:

```json
{"jsonrpc":"2.0","id":20,"method":"tools/call","params":{"name":"world_query","arguments":{"data":{},"filter":{"with":["bevy_egui::EguiContextSettings"],"without":[]},"strict":true,"port":15702}}}
```

`EguiContextSettings` is the reflectable required component on the primary Egui
context; `PrimaryEguiContext` itself is not reflected and therefore cannot be a
strict BRP query filter.

Wait for interpolation readiness before capturing the first reflected
`NetworkDemoStatus`. Compare the two position queries and all three resource
reads to prove input response, counter changes, and continued ticking.

When an MCP client is unavailable, direct BRP JSON-RPC provides a narrow
fallback:

```sh
curl -sS http://127.0.0.1:15702 \
  -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"world.get_resources","params":{"resource":"template_bevy::NetworkDemoStatus"}}'

curl -sS http://127.0.0.1:15702 \
  -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":2,"method":"brp_extras/send_keys","params":{"keys":["Space","ArrowRight"],"duration_ms":200}}'

curl -sS http://127.0.0.1:15702 \
  -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":3,"method":"brp_extras/screenshot","params":{"path":"/tmp/template-bevy.png"}}'
```

A complete smoke check reads status twice, proves ticks advance, injects input,
proves local actions and authoritative receipts increase, verifies the predicted
body responds, and reads status once more to catch executor stalls. The normal
mode screenshot should show the cyan predicted circle, orange interpolated box,
arena, and particles. On macOS, editor-mode BRP screenshots can be black and can
temporarily stall frame delivery even when Egui and BRP were healthy beforehand;
editor startup logs, the reflectable Egui context settings, and pre-screenshot
live status are the required proof in that case.

Wait for `input_timeline_synced: true` before injecting keys. The client uses
Lightyear's balanced input-delay policy so inputs arrive in time even though the
rendered client and authority run on independently scheduled executor threads.
It also keeps Winit updating continuously while unfocused; otherwise the
authority can advance while client prediction and short automated key presses
are paused.

Terminate only the process started for the smoke test. Check the PID owning the
BRP port before sending a signal; never use a broad process kill.

## AI project files

[`ai/AGENTS.md`](ai/AGENTS.md) is canonical. The generated root `AGENTS.md` and
`CLAUDE.md` are symlinks, as are tool-specific instruction/config paths. Edit
the tracked files under `ai/`, then run `nix run .#setup-ai`; do not hand-edit
generated links.

## License

MIT. See [LICENSE](LICENSE).
