use bevy::prelude::*;

mod collision;
mod debug;
mod game;
mod loading;
mod map;
mod menu;
mod mob;
mod net;
mod player;
mod ui;

#[derive(States, Default, Debug, Clone, Eq, PartialEq, Hash)]
pub enum AppState {
    #[default]
    MainMenu,
    InGame,
}

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
        .init_state::<AppState>()
        .add_plugins(bevy_egui::EguiPlugin)
        .add_plugins((
            net::NetworkPlugin,
            menu::MenuPlugin,
            game::GamePlugin,
            ui::ConsolePlugin,
            ui::InventoryPlugin,
            loading::LoadingPlugin,
            map::MapPlugin,
            collision::CollisionPlugin,
            player::PlayerPlugin,
            mob::MobPlugin,
            debug::DebugPlugin,
        ))
        .run();
}
