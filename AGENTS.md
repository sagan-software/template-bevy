# Project maintenance

Use ordinary Cargo independently of Nix. Nix supplies pinned compilers, native
libraries, and workflow tools. Preserve unrelated user changes.

## Dependency updates

Before implementation, verify the latest stable Bevy release, plugins, and Rust
toolchain against official release sources. Check plugin compatibility with
Bevy and the required targets before selecting versions.
Update manifests and lockfiles together, then validate generated projects.
Record the verification date, source links, selected versions, and compatibility
blockers in `docs/src/dependencies.md`. Treat template pins as reproducible
snapshots that require verification before use.

## Structure and documentation

Keep reusable domain code in `crates/` behind small plugin APIs.
Retain validated settings internally and parse textual values at ingress.
Keep imports flat with one item per statement. Run `cargo xtask imports`.

Keep README focused on generation, launch, controls, and links.
Document technical behavior in the mdBook under `docs/src/`.
When behavior changes, update its chapter and the generated README if applicable.
When adding a chapter, update `docs/src/SUMMARY.md`.
When recording a check, distinguish compilation, automated tests, and runtime evidence.

`CLAUDE.md` is a relative symlink to this file. Run `cargo xtask setup` after
generation because cargo-generate skips symlinks. Keep repository skills in
`.agents/skills` and Codex configuration in `.codex`.

## Worktree builds

Before creating or building worktrees, read `docs/src/builds.md`.
On Unix, use `python scripts/cargo-fast.py build --locked -p <package>` for focused builds.
On Windows, use ordinary Cargo.
Keep the compiler, lockfile, profile, features, and Rust flags consistent between workers.

Keep final target directories private to each worktree or worker lane.
The helper shares dependency intermediates only with a unique workspace compiler namespace and a validated dependency graph.
When workers build concurrently, give each worker a unique `--lane <name>` before the Cargo command.
Use `--cache-mode isolated` for commands that launch other compiler tooling.
Clippy, xtask and shell exports select isolated intermediates automatically.
Never replace these guards with one shared mutable Cargo directory.

Never clean another worker's cache or rewrite unchanged namespace launchers.
Keep stable domain boundaries in `crates/`; unchanged crates can reuse compiled artifacts.
Split crates only when timings show that the boundary reduces recompilation.
Run focused package checks while editing, then every required verification gate before handoff.
Record cold, cached-worktree, and edit-build timings with `--timings`.

## Verification

Run these checks after the final edit:

```sh
python -m unittest discover -s scripts -p 'test_*.py'
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo xtask check
nix run .#dylint
nix flake check
```

Run `cargo xtask generate-matrix` in a clean generation destination.
Verify normal and inspector launches, movement and boost, actual authority
receipts, native dimensions, and generated browser output before committing.
Build the book and inspect desktop and mobile rendering.
Record exact coverage gaps instead of describing compilation as behavioral proof.

Use the configured user Git identity. Commit only the requested changes after
verification. Inspect commit author, committer, message, and trailers before pushing.
