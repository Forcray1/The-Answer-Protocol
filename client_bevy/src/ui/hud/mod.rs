mod components;
mod setup;
mod systems;

pub(crate) use components::*;
pub(crate) use setup::*;
pub(crate) use systems::*;

use bevy::prelude::*;
use crate::AppState;
use crate::combat::CombatResource;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::LocalPlayerName;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerStats>()
            .add_systems(Startup, setup_hud_ui)
            .add_systems(
                Update,
                (handle_player_stats, update_hud_combat_visibility)
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(OnEnter(AppState::InGame), show_hud)
            .add_systems(OnExit(AppState::InGame), hide_hud)
            .add_systems(OnEnter(AppState::MainMenu), reset_ui_data_on_logout);
    }
}
