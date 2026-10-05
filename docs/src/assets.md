# Blender and Skein

Select Skein during generation to enable `game-assets` and Bevy glTF loading.
The asset plugin installs `bevy_skein::SkeinPlugin` after Bevy's glTF plugins.
It ingests reflected Rust components from Skein metadata on glTF entities.
The application owns BRP; Skein uses `handle_brp: false`.
When MCP is selected, Skein adds its BRP extensions without replacing remote methods.

Install the Blender extension using the
[Skein installation guide](https://bevyskein.dev/docs/installation).
Skein requires Blender 4.2 or newer. Blender 5.0 requires extension 0.1.13 or newer;
Blender 5.2 requires extension 0.1.16 or newer.
Register scene-authored component types with Bevy reflection before loading assets.
Export a glTF scene with Skein metadata, then load its scene through Bevy:

```rust,ignore
commands.spawn(SceneRoot(
    asset_server.load(GltfAssetLabel::Scene(0).from_asset("scene.glb")),
));
```

Keep assets under the project's `assets/` directory.
The starter does not include a Blender-authored scene or install Blender.
The ingestion test inserts actual Skein glTF metadata and checks that its reflected
component is created. A plugin-registration assertion alone does not prove ingestion.

Hanabi particles run in native networking mode. The browser starter uses WebGL2;
[Hanabi's browser support requires WebGPU](https://github.com/djeedai/bevy_hanabi).
Browser builds therefore omit Hanabi particles while retaining gameplay.
