//! Shared services for small Bevy games.
mod audio;
mod button;
mod plugin;
mod pointer;
mod safe_area;
mod save;
pub use audio::AudioSettings;
pub use audio::AudioUnlocked;
pub use audio::SoundChannel;
pub use audio::play_music;
pub use audio::play_sfx;
pub use button::FactoryButton;
pub use plugin::FactoryPlugin;
pub use pointer::FactorySet;
pub use pointer::PointerGesture;
pub use pointer::PointerKind;
pub use pointer::PointerTracker;
pub use safe_area::SafeAreaInsets;
pub use safe_area::SafeAreaRoot;
pub use safe_area::safe_area_root;
pub use safe_area::screen_button;
pub use save::Progress;
pub use save::SaveData;
pub use save::SaveError;
pub use save::SaveStore;

#[cfg(target_os = "android")]
pub use safe_area::set_android_insets;
