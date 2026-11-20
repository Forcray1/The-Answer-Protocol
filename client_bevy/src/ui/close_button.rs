use bevy::prelude::*;
use crate::AppState;
use crate::combat::CombatResource;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::LocalPlayerName;
use crate::ui::inventory::InventoryState;
use crate::ui::quest::QuestState;

#[derive(Component)]
pub struct CloseButton(pub String);

pub(crate) fn handle_close_buttons(
    mut interaction_query: Query<
        (&Interaction, &CloseButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut inv_state: ResMut<InventoryState>,
    mut quest_state: ResMut<QuestState>,
) {
    for (interaction, close_btn) in interaction_query.iter_mut() {
        if *interaction == Interaction::Pressed {
            if close_btn.0 == "inventory" {
                inv_state.open = false;
            } else if close_btn.0 == "quests" {
                quest_state.open = false;
            }
        }
    }
}
