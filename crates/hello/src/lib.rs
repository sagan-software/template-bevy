//! Portrait target challenge demonstrating FactoryPlugin.
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::ui_widgets::observe;
use bevy::window::WindowResolution;
use factory::AudioSettings;
use factory::AudioUnlocked;
use factory::FactoryPlugin;
use factory::SafeAreaInsets;
use factory::SaveData;
use factory::SaveStore;
use factory::play_music;
use factory::play_sfx;
use factory::safe_area_root;
use factory::screen_button;

/// Run on desktop, web, Android GameActivity, or iOS.
#[bevy_main]
pub fn main() {
    run_with_id("software.sagan.game-factory");
}

/// Launch the default demo from a desktop binary.
pub fn run() {
    main();
}

/// Run a generated game with its own persistent application identifier.
pub fn run_with_id(identifier: &str) {
    let store = SaveStore::new(identifier).expect("constant save identifier is valid");
    let (save, blocked) = match store.load() {
        Ok(save) => (save, false),
        Err(e) => {
            eprintln!("Save load failed: {e}");
            (SaveData::default(), true)
        }
    };
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Hello target".into(),
                resolution: WindowResolution::new(390, 780),
                canvas: Some("#game".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FactoryPlugin {
            save_store: Some(store),
            fallback_insets: SafeAreaInsets::default(),
        })
        .insert_resource(save.settings)
        .insert_resource(save)
        .insert_resource(Run {
            save_blocked: blocked,
            ..default()
        })
        .insert_resource(ClearColor(Color::srgb_u8(20, 38, 45)))
        .add_systems(Startup, setup)
        .add_systems(Update, (tick, render).chain())
        .run();
}

#[derive(Resource)]
struct Run {
    phase: Phase,
    score: u32,
    remaining: f32,
    save_blocked: bool,
    save_failed: bool,
    music: Option<Entity>,
}
impl Default for Run {
    fn default() -> Self {
        Self {
            phase: Phase::Ready,
            score: 0,
            remaining: 30.0,
            save_blocked: false,
            save_failed: false,
            music: None,
        }
    }
}
#[derive(PartialEq, Eq)]
enum Phase {
    Ready,
    Playing,
    Won,
    Lost,
}
#[derive(Component)]
struct Status;
#[derive(Component)]
struct Summary;
#[derive(Component)]
struct Target;
#[derive(Component)]
struct Start;
#[derive(Component)]
struct StartLabel;
#[derive(Component)]
struct SoundLabel;
#[derive(Resource)]
struct Sounds {
    tap: Handle<AudioSource>,
    win: Handle<AudioSource>,
    music: Handle<AudioSource>,
}

fn label(text: &str, size: f32) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
    )
}
fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.insert_resource(Sounds {
        tap: assets.load("audio/tap.wav"),
        win: assets.load("audio/win.wav"),
        music: assets.load("audio/music.wav"),
    });
    commands.spawn(safe_area_root()).with_children(|root| {
        root.spawn(Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|outer| {
            outer
                .spawn(Node {
                    width: percent(100),
                    max_width: px(430),
                    height: percent(100),
                    padding: UiRect::all(px(24)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(18),
                    ..default()
                })
                .with_children(|column| {
                    column.spawn((
                        label("Hello target", 32.0),
                        Node {
                            margin: UiRect::top(px(12)),
                            ..default()
                        },
                    ));
                    column.spawn((label("10 targets. 30 seconds.", 18.0), Status));
                    column
                        .spawn((
                            Node {
                                width: percent(100),
                                flex_grow: 1.0,
                                min_height: px(190),
                                ..default()
                            },
                            BackgroundColor(Color::srgb_u8(27, 51, 59)),
                        ))
                        .with_children(|field| {
                            field
                                .spawn(screen_button())
                                .insert((
                                    Target,
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: percent(40),
                                        top: percent(40),
                                        width: px(72),
                                        height: px(72),
                                        border_radius: BorderRadius::MAX,
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    },
                                    observe(hit),
                                ))
                                .with_children(|target| {
                                    target.spawn((
                                        label("+", 32.0),
                                        TextColor(Color::srgb_u8(20, 38, 45)),
                                    ));
                                });
                        });
                    column.spawn((label("Tap each orange target.", 18.0), Summary));
                    column
                        .spawn((screen_button(), Start, observe(start)))
                        .with_children(|b| {
                            b.spawn((
                                label("Start run", 22.0),
                                TextColor(Color::srgb_u8(20, 38, 45)),
                                StartLabel,
                            ));
                        });
                    column
                        .spawn(screen_button())
                        .insert((
                            BackgroundColor(Color::srgb_u8(42, 64, 71)),
                            observe(toggle_sound),
                        ))
                        .with_children(|b| {
                            b.spawn((label("Sound on", 18.0), SoundLabel));
                        });
                });
        });
    });
}
fn start(
    _: On<Activate>,
    mut run: ResMut<Run>,
    mut commands: Commands,
    sounds: Res<Sounds>,
    settings: Res<AudioSettings>,
    mut unlocked: ResMut<AudioUnlocked>,
) {
    if run.phase == Phase::Playing {
        return;
    }
    run.phase = Phase::Playing;
    run.score = 0;
    run.remaining = 30.0;
    unlocked.0 = true;
    if run.music.is_none() {
        run.music = play_music(&mut commands, sounds.music.clone(), *settings, &unlocked);
    }
}
fn hit(
    _: On<Activate>,
    mut run: ResMut<Run>,
    mut commands: Commands,
    sounds: Res<Sounds>,
    settings: Res<AudioSettings>,
    unlocked: Res<AudioUnlocked>,
    mut save: ResMut<SaveData>,
    store: Res<SaveStore>,
) {
    if run.phase != Phase::Playing {
        return;
    }
    run.score += 1;
    play_sfx(&mut commands, sounds.tap.clone(), *settings, &unlocked);
    if run.score == 10 {
        run.phase = Phase::Won;
        play_sfx(&mut commands, sounds.win.clone(), *settings, &unlocked);
        finish(&mut run, &mut save, &store, *settings);
    }
}
fn finish(run: &mut Run, save: &mut SaveData, store: &SaveStore, settings: AudioSettings) {
    save.progress.runs = save.progress.runs.saturating_add(1);
    save.progress.best_score = save.progress.best_score.max(run.score);
    save.settings = settings;
    if !run.save_blocked
        && let Err(e) = store.store(save)
    {
        warn!("Save write failed: {e}");
        run.save_failed = true;
    }
}
fn tick(
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut save: ResMut<SaveData>,
    store: Res<SaveStore>,
    settings: Res<AudioSettings>,
) {
    if run.phase == Phase::Playing {
        run.remaining = (run.remaining - time.delta_secs()).max(0.0);
        if run.remaining == 0.0 {
            run.phase = Phase::Lost;
            finish(&mut run, &mut save, &store, *settings);
        }
    }
}
fn toggle_sound(
    _: On<Activate>,
    mut settings: ResMut<AudioSettings>,
    mut save: ResMut<SaveData>,
    store: Res<SaveStore>,
    mut run: ResMut<Run>,
) {
    settings.muted = !settings.muted;
    save.settings = *settings;
    if !run.save_blocked
        && let Err(e) = store.store(&save)
    {
        warn!("Settings save failed: {e}");
        run.save_failed = true;
    }
}
#[allow(clippy::type_complexity)]
fn render(
    run: Res<Run>,
    save: Res<SaveData>,
    settings: Res<AudioSettings>,
    mut labels: ParamSet<(
        Query<&mut Text, With<Status>>,
        Query<&mut Text, With<Summary>>,
        Query<&mut Text, With<StartLabel>>,
        Query<&mut Text, With<SoundLabel>>,
    )>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<Target>>,
        Query<&mut Node, With<Start>>,
    )>,
) {
    let score = run.score;
    let seconds = run.remaining.ceil() as u32;
    for mut text in &mut labels.p0() {
        **text = if run.phase == Phase::Playing {
            format!("{score} / 10     {seconds}s")
        } else {
            "10 targets. 30 seconds.".into()
        };
    }
    for mut text in &mut labels.p1() {
        let best = save.progress.best_score;
        **text = match run.phase {
            Phase::Ready => "Tap each orange target.".into(),
            Phase::Playing => "Keep going.".into(),
            Phase::Won => format!("You win! 10 / 10. Best: {best}"),
            Phase::Lost => format!("Time's up. {score} / 10. Best: {best}"),
        };
        if run.save_blocked || run.save_failed {
            text.0.push_str(" Saving unavailable.");
        }
    }
    for mut text in &mut labels.p2() {
        **text = if run.phase == Phase::Ready {
            "Start run"
        } else {
            "Play again"
        }
        .into();
    }
    for mut text in &mut labels.p3() {
        **text = if settings.muted {
            "Sound off"
        } else {
            "Sound on"
        }
        .into();
    }
    for mut node in &mut nodes.p0() {
        node.display = if run.phase == Phase::Playing {
            Display::Flex
        } else {
            Display::None
        };
        let positions = [
            (40.0, 40.0),
            (12.0, 12.0),
            (65.0, 62.0),
            (15.0, 60.0),
            (60.0, 16.0),
            (38.0, 68.0),
            (8.0, 35.0),
            (63.0, 38.0),
            (35.0, 8.0),
            (35.0, 38.0),
        ];
        let (x, y) = positions[(score as usize).min(9)];
        node.left = percent(x);
        node.top = percent(y);
    }
    for mut node in &mut nodes.p1() {
        node.display = if run.phase == Phase::Playing {
            Display::None
        } else {
            Display::Flex
        };
    }
}

/// Called by the Android GameActivity WindowInsets listener. JNI arguments are opaque and never dereferenced.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_software_sagan_gamefactory_MainActivity_nativeInsets(
    _env: *mut std::ffi::c_void,
    _class: *mut std::ffi::c_void,
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
) {
    factory::set_android_insets(top, right, bottom, left);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str) -> (App, Entity, Entity, Entity) {
        let mut app = App::new();
        app.insert_resource(Run::default())
            .insert_resource(SaveData::default())
            .insert_resource(AudioSettings::default())
            .insert_resource(AudioUnlocked(false))
            .insert_resource(Time::<()>::default())
            .insert_resource(Sounds {
                tap: default(),
                win: default(),
                music: default(),
            })
            .insert_resource(SaveStore::new("hello-test").unwrap().with_directory(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
                    "../../target/hello-tests/{name}-{}",
                    std::process::id()
                )),
            ));
        let start = app.world_mut().spawn_empty().observe(start).id();
        let target = app.world_mut().spawn_empty().observe(hit).id();
        let sound = app.world_mut().spawn_empty().observe(toggle_sound).id();
        (app, start, target, sound)
    }

    #[test]
    fn win_persists_once_and_retry_resets_run() {
        let (mut app, start, target, sound) = app("win");
        app.world_mut().trigger(Activate { entity: target });
        assert_eq!(app.world().resource::<Run>().score, 0);
        app.world_mut().trigger(Activate { entity: start });
        let music = app.world().resource::<Run>().music;
        app.world_mut().trigger(Activate { entity: start });
        assert_eq!(app.world().resource::<Run>().music, music);
        for _ in 0..10 {
            app.world_mut().trigger(Activate { entity: target });
        }
        assert!(app.world().resource::<Run>().phase == Phase::Won);
        app.world_mut().trigger(Activate { entity: target });
        assert_eq!(app.world().resource::<SaveData>().progress.runs, 1);
        let saved = app.world().resource::<SaveStore>().load().unwrap();
        assert_eq!(saved.progress.best_score, 10);
        app.world_mut().trigger(Activate { entity: sound });
        assert!(
            app.world()
                .resource::<SaveStore>()
                .load()
                .unwrap()
                .settings
                .muted
        );
        app.world_mut().trigger(Activate { entity: start });
        let run = app.world().resource::<Run>();
        assert!(run.phase == Phase::Playing);
        assert_eq!(run.score, 0);
        assert_eq!(run.remaining, 30.0);
    }

    #[test]
    fn timeout_finishes_once_and_preserves_blocked_save() {
        let (mut app, start, target, sound) = app("timeout");
        app.world_mut().resource_mut::<Run>().save_blocked = true;
        app.world_mut().trigger(Activate { entity: start });
        app.world_mut().trigger(Activate { entity: target });
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(31));
        app.add_systems(Update, tick);
        app.update();
        assert!(app.world().resource::<Run>().phase == Phase::Lost);
        assert_eq!(app.world().resource::<SaveData>().progress.runs, 1);
        app.update();
        assert_eq!(app.world().resource::<SaveData>().progress.runs, 1);
        app.world_mut().trigger(Activate { entity: sound });
        assert_eq!(
            app.world().resource::<SaveStore>().load().unwrap(),
            SaveData::default()
        );
    }
}
