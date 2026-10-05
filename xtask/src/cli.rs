//! Closed command vocabulary for project maintenance.

use clap::Parser;
use clap::Subcommand;

/// Public command-line entry point.
#[derive(Debug, Parser)]
#[command(about = "Build and verify the game workspace")]
pub(crate) struct Cli {
    /// Requested project operation.
    #[command(subcommand)]
    pub(crate) command: Operation,
}

/// Supported operations, each selected explicitly.
#[derive(Debug, Subcommand)]
pub(crate) enum Operation {
    /// Run formatting, tests, lint, documentation and coverage checks.
    Check,
    /// Require one item per Rust use statement.
    Imports,
    /// Create the Claude instruction link while preserving owned files.
    Setup,
    /// Build the public mdBook.
    Book,
    /// Generate HTML, LCOV and JSON coverage reports.
    Coverage,
    /// Check each supported optional feature.
    Features,
    /// Generate and verify the onboarding configurations.
    GenerateMatrix,
}
