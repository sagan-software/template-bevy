//! Runs the portable maintenance CLI from the workspace root.

use std::process::Command;

/// Demonstrates discovering commands before running a project check.
fn main() -> std::io::Result<()> {
    let status = Command::new("cargo")
        .args(["run", "--manifest-path", "xtask/Cargo.toml", "--", "--help"])
        .status()?;
    assert!(status.success(), "maintenance help failed");
    Ok(())
}
