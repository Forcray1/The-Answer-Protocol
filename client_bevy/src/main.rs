use bevy::prelude::*;

mod board_quest_ui;
mod collision;
mod combat;
mod command_button;
mod command_menu;
mod debug;
mod game;
mod interaction;
mod loading;
mod map;
mod menu;
mod minimap;
mod mob;
mod net;
mod parse_layers;
mod player;
mod quest;
mod shop;
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
            ui::QuestPlugin,
            ui::HudPlugin,
            loading::LoadingPlugin,
            map::MapPlugin,
            collision::CollisionPlugin,
            player::PlayerPlugin,
            mob::MobPlugin,
            combat::CombatPlugin,
            debug::DebugPlugin,
            minimap::MinimapPlugin,
        ))
        .add_plugins(interaction::InteractionPlugin)
        .add_plugins(command_menu::CommandMenuPlugin)
        .add_plugins(quest::QuestTrackerPlugin)
        .add_plugins(shop::ShopPlugin)
        .add_plugins(board_quest_ui::BoardQuestPlugin)
        .run();
}
