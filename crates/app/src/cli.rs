//! Typed command-line startup choices.

use clap::Parser;
use std::path::Path;
use std::path::PathBuf;

/// Parses package identity and validated argument syntax through Clap.
#[derive(Clone, Debug, Default, Eq, PartialEq, Parser)]
#[command(name = env!("CARGO_PKG_NAME"), version, about)]
pub(crate) struct Cli {
    /// Loads timing settings from this TOML file.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    /// Opens the native world inspector.
    #[cfg(all(feature = "mcp", not(target_family = "wasm")))]
    #[arg(long, num_args = 0, default_missing_value = "enabled")]
    editor: Option<Editor>,
}

/// Presence-only capability that enables the native inspector.
#[cfg(all(feature = "mcp", not(target_family = "wasm")))]
#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub(crate) enum Editor {
    /// Adds the inspector plugins for this launch.
    Enabled,
}

impl Cli {
    /// Returns the requested configuration path without accessing the
    /// filesystem.
    #[must_use]
    pub(crate) fn config_path(&self) -> Option<&Path> {
        self.config.as_deref()
    }
    /// Reports the native inspector capability selected for this launch.
    #[must_use]
    pub(crate) const fn is_editor_enabled(&self) -> bool {
        #[cfg(all(feature = "mcp", not(target_family = "wasm")))]
        {
            self.editor.is_some()
        }
        #[cfg(not(all(feature = "mcp", not(target_family = "wasm"))))]
        {
            false
        }
    }
}

/// Verifies command-line parsing at the user-input boundary.
#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::CommandFactory;
    use clap::Parser;
    use clap::error::ErrorKind;
    use std::path::Path;

    /// Ensures Clap's generated command is internally consistent.
    #[test]
    fn command_definition_is_valid() {
        Cli::command().debug_assert();
    }

    /// Sources command identity, version, and summary from Cargo package
    /// metadata.
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
        assert!(!cli.is_editor_enabled());
    }

    /// Enables the editor through a runtime choice rather than a Cargo feature.
    #[test]
    #[cfg(all(feature = "mcp", not(target_family = "wasm")))]
    fn editor_flag_enables_the_runtime_editor() {
        let cli = Cli::try_parse_from(["template-bevy", "--editor"])
            .expect("the editor flag should be valid");

        assert!(cli.is_editor_enabled());
    }

    /// Rejects inspection when the selected build omits its native capability.
    #[test]
    #[cfg(not(all(feature = "mcp", not(target_family = "wasm"))))]
    fn editor_flag_is_absent_without_native_inspection() {
        let error = Cli::try_parse_from(["template-bevy", "--editor"])
            .expect_err("unavailable inspection must not be advertised");
        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
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
    #[cfg(all(feature = "mcp", not(target_family = "wasm")))]
    fn flags_are_order_independent() {
        let editor_first =
            Cli::try_parse_from(["template-bevy", "--editor", "--config", "first.toml"])
                .expect("editor-first arguments should be valid");
        let config_first =
            Cli::try_parse_from(["template-bevy", "--config", "second.toml", "--editor"])
                .expect("config-first arguments should be valid");

        assert!(editor_first.is_editor_enabled());
        assert_eq!(editor_first.config_path(), Some(Path::new("first.toml")));
        assert!(config_first.is_editor_enabled());
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

    /// Preserves omitted fields while applying explicitly supplied update
    /// flags.
    #[test]
    #[cfg(all(feature = "mcp", not(target_family = "wasm")))]
    fn parser_updates_only_explicit_choices() {
        let mut cli = Cli::try_parse_from(["template-bevy", "--config", "first.toml"])
            .expect("the initial configuration path should parse");

        cli.try_update_from(["template-bevy", "--editor"])
            .expect("the editor update should parse");

        assert_eq!(cli.config_path(), Some(Path::new("first.toml")));
        assert!(cli.is_editor_enabled());
    }
}
