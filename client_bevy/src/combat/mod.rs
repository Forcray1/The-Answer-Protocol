mod animation;
mod arena;
mod components;
mod events;
mod state;
mod turns;

pub(crate) use animation::*;
pub(crate) use arena::*;
pub(crate) use components::*;
pub(crate) use events::*;
pub(crate) use state::*;
pub(crate) use turns::*;

use crate::AppState;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::{LocalPlayerName, LocalPlayerSkin};
use crate::ui::PlayerStats;
use bevy::prelude::*;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CombatResource>()
            .add_systems(Startup, setup_combat_overlay)
            .add_systems(
                Update,
                (
                    listen_combat_events,
                    handle_combat_transition,
                    handle_combat_inputs,
                    advance_turn_sequence,
                    animate_combat_entities,
                    animate_player_charge,
                    animate_enemy_charge,
                    update_combat_hp_bars,
                    update_combat_menu_selection,
                    update_combat_btn_labels,
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
