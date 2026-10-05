//! External command-line contract for portable project maintenance.

use std::process::Command;
use tempfile::tempdir;

/// Advertises the supported operations at the public CLI boundary.
#[test]
fn help_lists_project_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("--help")
        .output()
        .expect("xtask must launch");
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("help must be UTF-8");
    for name in [
        "check",
        "setup",
        "book",
        "coverage",
        "features",
        "generate-matrix",
    ] {
        assert!(help.contains(name), "help omits {name}");
    }
}

/// Rejects malformed arguments before changing the selected directory.
#[test]
fn invalid_command_has_no_side_effects() {
    let directory = tempdir().expect("test directory must exist");
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .current_dir(directory.path())
        .arg("unknown-command")
        .output()
        .expect("xtask must launch");
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        directory
            .path()
            .read_dir()
            .expect("directory must be readable")
            .count(),
        0
    );
}

/// Creates the required Claude link without depending on Nix.
#[cfg(unix)]
#[test]
fn setup_creates_relative_claude_link_and_preserves_owned_file() {
    let directory = tempdir().expect("test directory must exist");
    std::fs::write(directory.path().join("AGENTS.md"), "project instructions")
        .expect("instructions must be writable");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_xtask"))
            .current_dir(directory.path())
            .arg("setup")
            .output()
            .expect("xtask must launch")
    };
    assert!(run().status.success());
    assert_eq!(
        std::fs::read_link(directory.path().join("CLAUDE.md"))
            .expect("Claude instructions must be linked"),
        std::path::Path::new("AGENTS.md")
    );
    assert!(run().status.success());
    std::fs::remove_file(directory.path().join("CLAUDE.md")).expect("test link must be removable");
    std::fs::write(directory.path().join("CLAUDE.md"), "owned instructions")
        .expect("file must be writable");
    assert!(run().status.success());
    assert_eq!(
        std::fs::read_to_string(directory.path().join("CLAUDE.md")).expect("owned file must exist"),
        "owned instructions"
    );
}

/// Rejects nested import groups through the actual maintenance command.
#[test]
fn import_check_rejects_nested_imports_and_accepts_flat_imports() {
    let directory = tempdir().expect("test directory must exist");
    let source = directory.path().join("crates/example/src");
    std::fs::create_dir_all(&source).expect("source directory must exist");
    let file = source.join("lib.rs");
    std::fs::write(&file, "use std::{path::Path, io};").expect("source must be writable");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_xtask"))
            .current_dir(directory.path())
            .arg("imports")
            .output()
            .expect("xtask must launch")
    };
    let rejected = run();
    assert_eq!(rejected.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("one item per use statement"));
    std::fs::write(&file, "use std::path::Path;\nuse std::io;").expect("source must be writable");
    assert!(run().status.success());
}

/// Rejects setup before it can create a link without its required instructions.
#[test]
fn setup_requires_agents_before_creating_link() {
    let directory = tempdir().expect("test directory must exist");
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .current_dir(directory.path())
        .arg("setup")
        .output()
        .expect("xtask must launch");
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("AGENTS.md is required"));
    assert!(!directory.path().join("CLAUDE.md").exists());
}

/// Repairs a dangling generated link without accessing its missing target.
#[cfg(unix)]
#[test]
fn setup_repairs_a_dangling_link() {
    let directory = tempdir().expect("test directory must exist");
    std::fs::write(directory.path().join("AGENTS.md"), "instructions")
        .expect("instructions must exist");
    std::os::unix::fs::symlink("missing.md", directory.path().join("CLAUDE.md"))
        .expect("link must be created");
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .current_dir(directory.path())
        .arg("setup")
        .output()
        .expect("xtask must launch");
    assert!(output.status.success());
    assert_eq!(
        std::fs::read_link(directory.path().join("CLAUDE.md")).expect("link must exist"),
        std::path::Path::new("AGENTS.md")
    );
}

/// Exercises portable gate ordering and child failure propagation without Nix.
#[cfg(unix)]
#[test]
fn check_runs_all_gates_and_stops_after_a_failed_child() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempdir().expect("test directory must exist");
    std::fs::write(directory.path().join("AGENTS.md"), "instructions")
        .expect("instructions must exist");
    let tools = directory.path().join("tools");
    std::fs::create_dir(&tools).expect("tool directory must exist");
    for program in ["cargo", "mdbook"] {
        let path = tools.join(program);
        std::fs::write(
            &path,
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> calls.log\nif [ \"$FAIL_GATE\" = \"$1\" ]; then exit 17; fi\n",
        )
        .expect("fake tool must exist");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("fake tool must be executable");
    }
    let run = |operation: &str, failure: &str| {
        Command::new(env!("CARGO_BIN_EXE_xtask"))
            .current_dir(directory.path())
            .env("PATH", &tools)
            .env("FAIL_GATE", failure)
            .arg(operation)
            .output()
            .expect("xtask must launch")
    };
    assert!(run("check", "").status.success());
    let calls = std::fs::read_to_string(directory.path().join("calls.log"))
        .expect("child calls must be recorded");
    assert!(calls.starts_with("fmt --all -- --check\nclippy --workspace --all-targets --all-features --locked -- -D warnings\ntest --workspace --locked\n"));
    assert!(calls.contains("check --workspace --locked --features dynamic_linking\n"));
    for features in [
        "dim2,mcp",
        "dim3,mcp",
        "dim3,networked,mcp",
        "dim2,dim3,networked,mcp",
    ] {
        let expected =
            format!("test -p game --locked --no-default-features --features {features}\n");
        assert!(
            calls.contains(&expected),
            "missing dimension gate: {features}"
        );
    }
    assert!(calls.contains("doc --workspace --no-deps --document-private-items --locked\n"));
    assert!(calls.contains("bench --workspace --locked --no-run\n"));
    assert!(calls.contains("build docs\n"));
    assert!(calls.contains("llvm-cov report --html --output-dir target/llvm-cov"));
    assert!(calls.contains("llvm-cov report --lcov --output-path target/llvm-cov/lcov.info"));
    assert!(calls.contains("llvm-cov report --json --output-path target/llvm-cov/coverage.json"));
    assert!(calls.contains("llvm-cov report --fail-under-lines 50"));
    std::fs::write(directory.path().join("calls.log"), "").expect("log must reset");
    let rejected = run("check", "clippy");
    assert_eq!(rejected.status.code(), Some(1));
    let calls = std::fs::read_to_string(directory.path().join("calls.log"))
        .expect("child calls must be recorded");
    assert_eq!(calls.lines().count(), 2);
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("failed with"));
    std::fs::write(directory.path().join("calls.log"), "").expect("log must reset");
    let rejected = run("features", "test");
    assert_eq!(rejected.status.code(), Some(1));
    let calls = std::fs::read_to_string(directory.path().join("calls.log"))
        .expect("dimension calls must be recorded");
    assert_eq!(calls.lines().count(), 5);
    assert!(calls.ends_with("test -p game --locked --no-default-features --features dim2,mcp\n"));
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("failed with"));
}
