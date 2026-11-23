mod board;
mod components;
mod ground_items;
mod mobs;
mod prompt;

pub(crate) use board::*;
pub(crate) use components::*;
pub(crate) use ground_items::*;
pub(crate) use mobs::*;
pub(crate) use prompt::*;

use bevy::prelude::*;
use crate::AppState;

pub struct MobPlugin;

impl Plugin for MobPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InteractableTarget>()
            .init_resource::<GroundItemTarget>()
            .add_systems(Startup, setup_interaction_ui)
            .add_systems(OnEnter(AppState::MainMenu), clear_mobs_on_logout)
            .add_systems(
                Update,
                (
                    handle_mob_events,
                    handle_board_spawn_events,
                    animate_mobs,
                    cleanup_mobs_on_room_change,
                    handle_ground_item_events,
                    update_ground_item_target_impl,
                    interaction_system,
                    handle_interaction_input,
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
