//! Runtime logging policy without a direct dependency on the log facade.

use bevy::log::Level;
use bevy::log::LogPlugin;

/// Selects the default logging level for the current compilation profile.
pub(crate) fn plugin() -> LogPlugin {
    profile_plugin(cfg!(debug_assertions))
}

/// Keeps release launches at warning level while development retains
/// information.
fn profile_plugin(is_debug_build: bool) -> LogPlugin {
    // Retain every Bevy filter and sink explicitly while selecting the default level.
    let defaults = LogPlugin::default();
    LogPlugin {
        level: if is_debug_build {
            Level::INFO
        } else {
            Level::WARN
        },
        filter: defaults.filter,
        custom_layer: defaults.custom_layer,
        fmt_layer: defaults.fmt_layer,
    }
}

/// Verifies each profile without requiring two complete renderer builds.
#[cfg(test)]
mod tests {
    use super::profile_plugin;
    use bevy::log::Level;

    /// Retains the release warning policy after removing the direct log
    /// dependency.
    #[test]
    fn profiles_select_the_documented_default_levels() {
        assert_eq!(profile_plugin(true).level, Level::INFO);
        assert_eq!(profile_plugin(false).level, Level::WARN);
    }
}
