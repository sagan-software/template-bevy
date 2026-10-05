# Repository review

Source: OBS recording `2026-10-05 10-28-46.mkv`, 23 minutes 39.733 seconds.
The complete timestamped transcript is retained beside the recording.
Related lint feedback: recording `2026-10-05 09-33-09.mkv`, 09:28–13:07.

Priority and sequencing follow dependencies. P0 covers compatibility, generation,
physics, and final verification. P1 covers structure, configuration, tooling, and
performance defaults. P2 covers imports, logging, presentation, and documentation.

1. P0 [08:03–08:27, 09:19–10:03, 11:54–12:53] Verify stable Rust, Bevy and plugins; synchronize Rust toolchain, Cargo MSRV and Clippy MSRV; update Cargo and Nix locks. Check actual compilation and document compatibility blockers.
2. P0 [00:26–03:01, 12:58–13:23, 15:49–16:36] Add cargo-generate onboarding for project name, 2D/3D/both, networking, Blender/Skein, MCP and browser/Trunk. Test generated projects, omitted dependencies, names, and browser output. Keep one repository.
3. P0 [22:27–22:51] Replace Avian with Rapier in both dimensions. Test physical movement and preserve authoritative networking proof where enabled.
4. P1 [16:48–17:32, 18:46–19:45, 22:18–22:25] Split purpose-driven crates under crates/. Expose small plugin APIs with builder configuration; rename TemplateBevyPlugin. Test external plugin composition and private behavior.
5. P1 [20:07–22:13] Replace unit-suffixed configuration fields with unit-bearing values, including tick rate and replication duration. Store validated typed values; test parsing, serialization, defaults, errors and bounds. Inspect video for exact examples.
6. P1 [05:41–07:04] Remove ai/. Track root AGENTS.md, symlink CLAUDE.md, use .agents/skills and .codex configuration. Support ordinary Cargo use independently of Nix. Verify links after generation.
7. P1 [07:18–08:03, 08:28–08:48] Remove dprint and its pins/nix folder. Retain rustfmt/treefmt; reduce flake duplication without dropping validation. Search for removed names and run formatting.
8. P1 [10:09–11:53] Integrate public Dylints through its current recommended quick start, without local paths. Test expected finding and passing replacement using the user command.
9. P1 [13:23–13:59] Group warning, justified allowance and disabled lint settings; document priority and audit disabled lints. Apply the earlier review's exact instructions to rustc, rustdoc and Clippy.
10. If supported, move Cargo profiles into .cargo/config.toml. Verify effective profile settings. P1 [14:07–14:43].
11. P1 [08:54–09:16, 14:48–15:49] Add flamegraph/performance tools; review official Bevy compile and browser optimization guidance. Preserve benchmarks and optional profiling; verify tools/builds.
12. P2 [17:45–18:36] Use flat one-item imports, enforce with a supported lint or formatting check. Keep stable Rust.
13. If the direct log dependency is unnecessary after auditing Bevy logging, remove it. Preserve chosen release filtering explicitly. P2 [22:57–23:08].
14. P2 [03:09–05:08] Shorten README, provide generated-project README with logo, truthful badges, actual screenshot and GIF. Research established examples and inspect rendered output.
15. P2 [05:08–05:36] Add extensible public mdBook under docs/; move technical material there and document maintenance in AGENTS/CLAUDE. Build book and verify links.
16. P0 final [user request] Run exact required gates after final edits, generation matrix, normal/editor/native/browser runtime paths, coverage, personal lints, complete Nix checks. Audit applicable instructions and commit identity. Commit only verified changes, push and verify remote commit.

Invariant scratchpad:

- Dimension is a closed 2D/3D/both choice; networking, assets, MCP and web are independent explicit capabilities.
- Omitted integrations must not require their dependencies, plugins, CLI flags or tools.
- Configuration validates at ingress. Tick duration derives from frequency; snapshots retain Duration.
- Invalid configuration returns a source-preserving parse error before application startup.
- Authority receipt and interpolation readiness require actual receive-side evidence.
- Browser builds cannot start native server threads, inspector HTTP listeners or unsupported tools.
- Nix is an optional reproducible environment; ordinary Cargo remains usable.
- Generated project naming covers manifests, imports, tooling and documentation.
- No commit precedes successful user-path evidence.

Required gates: cargo fmt --all -- --check; cargo test; cargo clippy --all-targets --all-features -- -D warnings; applicable personal-lint command; configured coverage command; nix flake check; generated-project builds/runtime; book build. A conflicting optional feature combination must be resolved explicitly, never described as verified when skipped.
