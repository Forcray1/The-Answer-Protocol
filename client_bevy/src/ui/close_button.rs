use bevy::prelude::*;
use crate::ui::inventory::InventoryState;
use crate::ui::quest::QuestState;

#[derive(Component)]
pub struct CloseButton(pub String);

// Named query types: they keep the system signatures short and readable
// (a long inline Query type triggers clippy::type_complexity).
type CloseButtonQuery<'w, 's> = Query<'w, 's, (&'static Interaction, &'static CloseButton), (Changed<Interaction>, With<Button>)>;

pub(crate) fn handle_close_buttons(
    mut interaction_query: CloseButtonQuery,
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
