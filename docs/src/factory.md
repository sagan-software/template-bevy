# Factory games and packaging

Generate a free-launch, portrait game from the local template:

```sh
mkdir -p target/generated
nix develop -c cargo generate --path . --name game-factory --destination target/generated \
  --no-workspace --vcs none --silent --allow-commands \
  --define dimension=2d --define factory=true --define networked=false \
  --define skein=false --define mcp=false --define web=true
cd target/generated/game-factory
nix develop path:. -c cargo xtask setup
nix develop path:. -c cargo run -p game-factory
```

Tap or click Start, then hit ten orange targets within thirty seconds. The result
screen supports replay. Sound can be muted. The demo uses shapes, an original
SVG icon, and procedurally generated WAV audio. Its source follows the repository
MIT license. Select `factory=false` for the original keyboard physics starter.

The reusable API is documented in [Adopting the factory](adopting.md).
`FactoryPlugin` exposes settings and progress storage explicitly; it never
replaces corrupt or future-version saves. Native saves use the application data
directory, Android uses internal app storage, and web uses localStorage.
Browser storage can be unavailable or cleared by the player.

## Web and itch

```sh
nix develop path:. -c bash scripts/web.sh
nix develop path:. -c bash scripts/serve.sh
curl -f http://127.0.0.1:8761/
curl -f -o /dev/null http://127.0.0.1:8761/game_factory_bg.wasm
```

The disk preflight requires fifty GiB or ten percent free after expected output.
An explicitly authorized local exception can set `FACTORY_DISK_RESERVE_GIB`.

The release build selects WebGL2, runs wasm-bindgen and wasm-opt, copies assets,
and writes `dist/web/` and `dist/game-factory-web.zip`. The zip has `index.html`
at its root. The server binds to `0.0.0.0:8761` and records its PID and log in
`local/`. Before replacing a listener, identify its owner and stop only this
template's previous server.

`bash scripts/push-itch.sh` exits without publishing when `BUTLER_API_KEY` is
unset. Publishing also requires an explicit `ITCH_TARGET` and Butler installation.
Running it publishes externally; obtain Bill's authorization first. CI never
runs it. First launch uses itch pay-what-you-want and free mobile listings.

## Icons and audio

```sh
nix develop -c python3 scripts/icons.py
nix develop -c python3 scripts/audio.py
```

`assets/icon.svg` generates favicon and manifest icons, Android density resources,
and the iOS AppIcon catalog. Store artwork needs Bill's final design and review.
The generated audio is small and original. Keep asset paths relative to the
application asset directory.

## Android

```sh
nix develop .#android -c bash scripts/android.sh
```

The flake supplies Android SDK 35, NDK 27.2, Java, Gradle, and cargo-ndk. The script
builds arm64-v8a Rust code, then debug APK and AAB through Gradle GameActivity.
Outputs are `dist/android/game-factory-debug.apk` and `game-factory-debug.aab`.
The debug keystore and Gradle caches stay under `local/`. No device installation
is part of this workflow. A debug AAB cannot be uploaded as a production release.

Release signing reads `ANDROID_KEYSTORE`, `ANDROID_STORE_PASSWORD`,
`ANDROID_KEY_ALIAS`, and `ANDROID_KEY_PASSWORD`. Bill must provide his own key
and credentials. No release key is generated. Set unique package identifiers,
version codes, and the currently required target SDK before store submission.

## iOS

Use macOS with Xcode and Nix. Open `platform/ios/game-factory.xcodeproj` and select
the `hello` scheme. `build_rust_deps.sh` builds the matching Rust architecture
and copies the executable into the app bundle. The shared scheme supports
unsigned simulator compilation:

```sh
xcodebuild -project platform/ios/game-factory.xcodeproj -scheme hello \
  -configuration Debug -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' \
  CODE_SIGNING_ALLOWED=NO build
```

Device archives require Bill's Apple team, bundle identifier, provisioning, and
signing setup. This Linux host cannot build or validate Xcode output. iOS safe
areas currently require configured fallback insets or a host resource update.
Validate portrait layout, icons, launch screen, audio, and storage on devices.

## Verification boundary

Required gates remain in `AGENTS.md`. Recheck generation in a fresh directory with
`cargo xtask generate-matrix --destination target/generated-final`; existing projects
are preserved and rejected rather than overwritten. The new Rust modules include tests for save
round-trip, migration, malformed data, identifiers, file errors, gesture thresholds,
cancellation, audio settings, inset normalization, win, timeout, retry, and mute persistence. Platform Java/JNI wiring,
Web Audio unlock, localStorage failures, iOS packaging, and rendered game outcomes
need platform runtime evidence. Compilation does not cover those paths.

See [the store checklist](../STORE-CHECKLIST.md) before shipping.

The web script matches the wasm-bindgen CLI to Cargo.lock. If the devshell CLI differs, it builds the matching CLI with Cargo inside the devshell and stores it under `local/tools`; compiler output stays in `target/tools-build`.
