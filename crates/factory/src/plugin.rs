//! Plugin configuration and scheduling.
use crate::AudioSettings;
use crate::AudioUnlocked;
use crate::FactorySet;
use crate::PointerGesture;
use crate::PointerTracker;
use crate::SafeAreaInsets;
use crate::SaveStore;
use bevy::prelude::*;

/// Install pointer, safe-area and master audio services after DefaultPlugins.
/// Persistence is explicit through SaveStore so games can handle errors without data loss.
#[derive(Default)]
pub struct FactoryPlugin {
    /// Insets used until the platform provides values. iOS hosts can update the resource.
    pub fallback_insets: SafeAreaInsets,
    /// Optional persistent app identity. Each game must use a different identifier.
    pub save_store: Option<SaveStore>,
}
impl Plugin for FactoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PointerTracker>()
            .init_resource::<bevy::ui::UiStack>()
            .init_resource::<AudioSettings>()
            .init_resource::<AudioUnlocked>()
            .insert_resource(self.fallback_insets)
            .add_message::<PointerGesture>()
            .add_systems(
                PreUpdate,
                crate::pointer::collect_input
                    .after(bevy::input::InputSystems)
                    .in_set(FactorySet::Input),
            )
            .add_systems(
                Update,
                (
                    crate::safe_area::update_insets,
                    crate::audio::sync_volume,
                    crate::button::activate_buttons,
                ),
            );
        if let Some(store) = &self.save_store {
            app.insert_resource(store.clone());
        }
    }
}
