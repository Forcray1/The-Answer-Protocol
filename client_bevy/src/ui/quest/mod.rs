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
use crate::ui::close_button::handle_close_buttons;

// Quest Journal UI  (U)

pub struct QuestPlugin;

impl Plugin for QuestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<QuestState>()
            .init_resource::<SelectedQuest>()
            .add_systems(Startup, setup_quest_ui)
            .add_systems(
                Update,
                (
                    toggle_quest_ui,
                    handle_quest_data,
                    handle_quest_selection,
                    handle_close_buttons,
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
