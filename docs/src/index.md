# Bevy game template

Generate a starter for 2D, 3D, or both dimensions, with optional networking,
Blender/Skein assets, MCP inspection, and browser builds.

```sh
cargo generate --git https://github.com/sagan-software/template-bevy \
  --name my-game --no-workspace --allow-commands
cd my-game
cargo xtask setup
cargo run -p my-game
```

Install cargo-generate 0.25.0 or newer before generating.
The hook resolves a new Cargo lockfile after applying project choices.
When generating inside an existing workspace, `--no-workspace` preserves its manifest.
The generated application package and browser binary use the selected project name.

Local mode needs no Lightyear authority. Networking mode demonstrates predicted
player movement and authoritative snapshot interpolation through an in-process transport.
Use WASD or arrows to move and Space to boost.
Networking mode also accepts the left gamepad stick and south button.

![Template networked 2D example](media/demo.gif)

The generated README includes starter media from this example.
Replace the media, logo, badges, and documentation with your game's verified content.
Technical guides cover [architecture](architecture.md), [configuration](configuration.md),
[development](development.md), and [Blender assets](assets.md).
