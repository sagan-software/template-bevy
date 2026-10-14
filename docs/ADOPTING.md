# Adopt the factory plugin

`crates/factory` is the reusable Bevy 0.20 service crate. It owns pointer gestures,
safe-area padding, audio controls, and versioned persistence. Gameplay stays in
the adopting game. `crates/hello` demonstrates a portrait run with ten targets,
a thirty-second timer, win and lose screens, and replay.

Add a path dependency, then install `FactoryPlugin` after `DefaultPlugins`:

```toml
factory = { path = "../template-bevy/crates/factory" }
```

```rust,ignore
app.add_plugins(factory::FactoryPlugin {
    fallback_insets: factory::SafeAreaInsets::default(),
    save_store: Some(factory::SaveStore::new("software.sagan.your-game")?),
});
```

Use a unique save identifier for each game. `SaveStore::load` returns defaults
only when no save exists. Corrupt, incomplete, and future-version saves return
errors. The demo preserves those files and blocks further writes. Handle errors
before replacing player progress. `SaveData::encode` and `decode` implement
version 2 and the documented version 1 migration. Extend progress by adding a
schema migration before changing its shape.

Read `PointerGesture` messages after `FactorySet::Input` in `PreUpdate`.
Positions and deltas use logical window pixels. A tap moves less than twelve
pixels and releases before half a second. A long press emits once at half a
second. A drag starts at twelve pixels and emits displacement from the press origin. Cancellation
emits no tap. Touch identifiers remain distinct; mouse uses a reserved identifier.
Use `screen_button()` with Bevy `On<Activate>` observers for tap actions. The helper activates on release and ignores drags and long presses.
The default minimum button size is 64 logical pixels.

Spawn `safe_area_root()` as the UI root. Place normal layout spacing inside it.
Web pages must include the CSS probe and `viewport-fit=cover` from `web/index.html`.
Android requires `MainActivity.java` and the JNI export in `crates/hello`.
iOS currently uses `fallback_insets`; its host can update `SafeAreaInsets` at
runtime. Test notched devices and rotation before release. This fallback is
not an automatic UIKit safe-area bridge.

`play_sfx` and `play_music` return the playback entity after the first pointer
press. Despawn the music entity to stop it. `AudioSettings` applies master volume
and mute to every factory sound, including existing sinks. Copy the AudioContext
resume handler from `web/index.html` into custom web shells. Browser autoplay
must be tested with a fresh browser session.

Select `factory=true` during cargo-generate for the touch demo. Select false for
the original physics starter. Enable the app `factory` feature to add services
to that starter. Keep `networked=false` and `skein=false` until compatible
releases exist; see the dependency chapter. No generated game includes billing,
ads, a paywall, or license checks.

Build and packaging commands are in [the factory chapter](src/factory.md).
Platform application identifiers are placeholders. Change Android, iOS, manifest,
and save identifiers together before shipping an adopting game.
