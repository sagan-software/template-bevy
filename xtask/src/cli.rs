//! Closed command vocabulary for project maintenance.

use clap::Parser;
use clap::Subcommand;
use std::path::PathBuf;

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
    GenerateMatrix {
        /// Fresh directory for generated validation projects.
        #[arg(long, default_value = "target/generated")]
        destination: PathBuf,
    },
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use super::Operation;
    use clap::Parser;
    use std::path::Path;

    /// Matrix runs preserve previous generated projects by choosing a fresh destination.
    #[test]
    fn matrix_destination_defaults_and_overrides() {
        for (arguments, expected) in [
            (vec!["xtask", "generate-matrix"], "target/generated"),
            (
                vec![
                    "xtask",
                    "generate-matrix",
                    "--destination",
                    "target/generated-final",
                ],
                "target/generated-final",
            ),
        ] {
            let Operation::GenerateMatrix { destination } = Cli::try_parse_from(arguments)
                .expect("valid matrix arguments")
                .command
            else {
                panic!("expected generation operation");
            };
            assert_eq!(destination, Path::new(expected));
        }
    }
}
