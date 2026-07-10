use bevy::prelude::*;

mod game;
mod map;
mod net;
mod player;
mod ui;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/../sprites").to_string(),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins((
            net::NetworkPlugin,
            game::GamePlugin,
            ui::ConsolePlugin,
            map::MapPlugin,
            player::PlayerPlugin,
        ))
        .run();
}
