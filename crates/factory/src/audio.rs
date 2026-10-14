//! Shared sound settings and playback helpers.
use bevy::audio::Volume;
use bevy::prelude::*;
use serde::Deserialize;
use serde::Serialize;

/// Persistent master controls. Volume is linear in the range zero to one.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AudioSettings {
    /// Linear master volume.
    pub master: f32,
    /// Silence every factory sound, including already playing music.
    pub muted: bool,
}
impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            master: 0.7,
            muted: false,
        }
    }
}
impl AudioSettings {
    /// Effective bounded volume, including mute and invalid-value handling.
    pub fn volume(self) -> f32 {
        if self.muted || !self.master.is_finite() {
            0.0
        } else {
            self.master.clamp(0.0, 1.0)
        }
    }
}

/// True after the first touch or mouse press. Browser audio resumes in a DOM gesture handler.
#[derive(Resource, Default)]
pub struct AudioUnlocked(pub bool);

/// Marks sounds controlled by the master settings.
#[derive(Component)]
pub enum SoundChannel {
    Sfx,
    Music,
}

/// Play one effect after the first interaction. Returns None before audio unlock.
pub fn play_sfx(
    commands: &mut Commands,
    source: Handle<AudioSource>,
    settings: AudioSettings,
    unlocked: &AudioUnlocked,
) -> Option<Entity> {
    unlocked.0.then(|| {
        commands
            .spawn((
                AudioPlayer::new(source),
                PlaybackSettings::DESPAWN.with_volume(Volume::Linear(settings.volume())),
                SoundChannel::Sfx,
            ))
            .id()
    })
}
/// Loop music after the first interaction. Despawn the returned entity to stop it.
pub fn play_music(
    commands: &mut Commands,
    source: Handle<AudioSource>,
    settings: AudioSettings,
    unlocked: &AudioUnlocked,
) -> Option<Entity> {
    unlocked.0.then(|| {
        commands
            .spawn((
                AudioPlayer::new(source),
                PlaybackSettings::LOOP.with_volume(Volume::Linear(settings.volume())),
                SoundChannel::Music,
            ))
            .id()
    })
}
pub(crate) fn sync_volume(
    settings: Res<AudioSettings>,
    mut sinks: Query<&mut AudioSink, With<SoundChannel>>,
) {
    for mut sink in &mut sinks {
        sink.set_volume(Volume::Linear(settings.volume()));
    }
}
