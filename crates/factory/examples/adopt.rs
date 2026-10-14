use bevy::prelude::*;
use factory::FactoryPlugin;
fn main() {
    App::new()
        .add_plugins((DefaultPlugins, FactoryPlugin::default()))
        .run();
}
