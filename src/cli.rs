//! Command-line arguments accepted by the demonstration binary.
//!
//! Command-line parsing stays separate from validated runtime configuration:
//! [`Cli`] describes what the user typed, while [`crate::Config`] describes the
//! settings the application can safely use.

use clap::{
    Arg, ArgAction, ArgMatches, Command, CommandFactory, Error, FromArgMatches, Parser,
    value_parser,
};
use std::path::{Path, PathBuf};

/// Selects optional startup behavior without changing the compiled game.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Cli {
    /// Path to an explicitly requested TOML configuration file.
    config: Option<PathBuf>,

    /// Enables the runtime world inspector included in every build.
    editor: bool,
}

impl Cli {
    /// Returns the explicitly requested configuration path, when supplied.
    #[must_use]
    pub fn config_path(&self) -> Option<&Path> {
        self.config.as_deref()
    }

    /// Reports whether the runtime editor should be installed.
    #[must_use]
    pub const fn editor_enabled(&self) -> bool {
        self.editor
    }
}

/// Builds the Clap command used for both initial parsing and partial updates.
impl CommandFactory for Cli {
    /// Describes the complete command using package metadata supplied by Cargo.
    fn command() -> Command {
        Command::new(env!("CARGO_PKG_NAME"))
            .version(env!("CARGO_PKG_VERSION"))
            .about(env!("CARGO_PKG_DESCRIPTION"))
            .arg(
                Arg::new("config")
                    .long("config")
                    .value_name("PATH")
                    .value_parser(value_parser!(PathBuf)),
            )
            .arg(Arg::new("editor").long("editor").action(ArgAction::SetTrue))
    }

    /// Reuses the same optional arguments when updating an existing value.
    fn command_for_update() -> Command {
        Self::command()
    }
}

/// Converts Clap's validated matches into the template's typed startup choices.
impl FromArgMatches for Cli {
    /// Creates a fresh value, using ordinary defaults for omitted optional flags.
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, Error> {
        Ok(Self {
            config: matches.get_one::<PathBuf>("config").cloned(),
            editor: matches.get_flag("editor"),
        })
    }

    /// Applies only explicitly supplied options to an existing value.
    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), Error> {
        // An absent config flag preserves the caller's selected file; this is
        // the partial-update behavior promised by Clap's `Parser` interface.
        if let Some(config) = matches.get_one::<PathBuf>("config") {
            self.config = Some(config.clone());
        }

        // A boolean presence flag can turn the editor on but has no implicit
        // inverse that would unexpectedly disable an existing runtime choice.
        if matches.get_flag("editor") {
            self.editor = true;
        }
        Ok(())
    }
}

/// Enables Clap's standard parse and update convenience methods for [`Cli`].
impl Parser for Cli {}

/// Verifies command-line parsing at the user-input boundary.
#[cfg(all(test, not(coverage)))]
mod tests {
    use super::Cli;
    use clap::{CommandFactory, Parser, error::ErrorKind};
    use std::path::Path;

    /// Ensures Clap's generated command is internally consistent.
    #[test]
    fn command_definition_is_valid() {
        Cli::command().debug_assert();
    }

    /// Sources command identity, version, and summary from Cargo package metadata.
    #[test]
    fn command_metadata_matches_the_package() {
        let command = Cli::command();

        assert_eq!(command.get_name(), env!("CARGO_PKG_NAME"));
        assert_eq!(command.get_version(), Some(env!("CARGO_PKG_VERSION")));
        assert_eq!(
            command.get_about().map(ToString::to_string),
            Some(env!("CARGO_PKG_DESCRIPTION").to_owned())
        );
    }

    /// Keeps the no-argument launch free from implicit filesystem access.
    #[test]
    fn no_arguments_use_runtime_defaults() {
        let cli =
            Cli::try_parse_from(["template-bevy"]).expect("the empty command line should be valid");

        assert_eq!(cli.config_path(), None);
        assert!(!cli.editor_enabled());
    }

    /// Enables the editor through a runtime choice rather than a Cargo feature.
    #[test]
    fn editor_flag_enables_the_runtime_editor() {
        let cli = Cli::try_parse_from(["template-bevy", "--editor"])
            .expect("the editor flag should be valid");

        assert!(cli.editor_enabled());
    }

    /// Preserves the exact path supplied by the caller.
    #[test]
    fn config_flag_records_the_requested_path() {
        let cli = Cli::try_parse_from(["template-bevy", "--config", "demo.toml"])
            .expect("a configuration path should be valid");

        assert_eq!(cli.config_path(), Some(Path::new("demo.toml")));
    }

    /// Accepts independent flags in either order.
    #[test]
    fn flags_are_order_independent() {
        let editor_first =
            Cli::try_parse_from(["template-bevy", "--editor", "--config", "first.toml"])
                .expect("editor-first arguments should be valid");
        let config_first =
            Cli::try_parse_from(["template-bevy", "--config", "second.toml", "--editor"])
                .expect("config-first arguments should be valid");

        assert!(editor_first.editor_enabled());
        assert_eq!(editor_first.config_path(), Some(Path::new("first.toml")));
        assert!(config_first.editor_enabled());
        assert_eq!(config_first.config_path(), Some(Path::new("second.toml")));
    }

    /// Rejects options the template does not advertise.
    #[test]
    fn unknown_flag_is_rejected() {
        let error = Cli::try_parse_from(["template-bevy", "--unknown"])
            .expect_err("unknown flags must not be silently ignored");

        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
    }

    /// Rejects a configuration flag that omits its required path.
    #[test]
    fn missing_config_value_is_rejected() {
        let error = Cli::try_parse_from(["template-bevy", "--config"])
            .expect_err("the config flag requires a path");

        assert_eq!(error.kind(), ErrorKind::InvalidValue);
    }

    /// Preserves omitted fields while applying explicitly supplied update flags.
    #[test]
    fn parser_updates_only_explicit_choices() {
        let mut cli = Cli::try_parse_from(["template-bevy", "--config", "first.toml"])
            .expect("the initial configuration path should parse");

        cli.try_update_from(["template-bevy", "--editor"])
            .expect("the editor update should parse");

        assert_eq!(cli.config_path(), Some(Path::new("first.toml")));
        assert!(cli.editor_enabled());
    }
}
