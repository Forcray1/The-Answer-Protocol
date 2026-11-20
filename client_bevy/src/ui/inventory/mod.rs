mod actions;
mod components;
mod equipment;
mod grid;
mod setup;
mod stats;
mod style;
mod tabs;

pub(crate) use actions::*;
pub(crate) use components::*;
pub(crate) use equipment::*;
pub(crate) use grid::*;
pub(crate) use setup::*;
pub(crate) use stats::*;
pub(crate) use style::*;
pub(crate) use tabs::*;

use bevy::prelude::*;
use crate::AppState;
use crate::combat::CombatResource;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::LocalPlayerName;

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryState>()
            .init_resource::<SelectedItem>()
            .add_systems(Startup, setup_inventory_ui)
            .add_systems(
                Update,
                (
                    toggle_inventory,
                    handle_inventory_data,
                    handle_equip_data,
                    handle_slot_interactions,
                    handle_action_btn,
                    handle_drop_btn,
                    handle_char_stats,
                    handle_tab_click,
                    update_tab_highlight,
                    update_silhouette,
                    handle_gold_event,
                    handle_stat_points_event,
                    handle_statup_click,
                    handle_use_item_btn,
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
