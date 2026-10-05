//! Portable maintenance commands for the generated game workspace.
//!
//! The typed CLI selects setup, import checks, generation, documentation, and
//! verification workflows. Operations preserve user-owned instruction files and
//! propagate failed child commands. Import checks parse Rust syntax in stable
//! path order. Start with `cli` for command choices and `operations` for their
//! execution; `imports` owns the source-shape boundary.

mod cli;
mod imports;
mod operations;

use self::cli::Cli;
use clap::Parser;
use std::io::Write;
use std::process::ExitCode;

/// Validates arguments before executing the selected maintenance operation.
fn main() -> ExitCode {
    // Clap rejects invalid commands before any project mutation.
    let operation = Cli::parse().command;
    // Propagate workflow failure through the process exit status.
    match operations::execute(operation) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _diagnostic_result = std::io::stderr().write_fmt(format_args!("{error}\n"));
            ExitCode::FAILURE
        }
    }
}
