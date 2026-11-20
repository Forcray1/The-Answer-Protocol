use bevy::prelude::*;
use crate::AppState;
use crate::combat::CombatResource;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::LocalPlayerName;
use crate::ui::inventory::components::{InventoryGrid, InventorySlot, InventoryState, InventoryTabBtn};
use crate::ui::inventory::grid::rebuild_grid;
use crate::ui::inventory::style::{BORDER_GOLD, TAB_ACTIVE_BG, TAB_IDLE_BG, TEXT_DIM, TITLE_COLOR};

/// Switches the active tab and re-filters the cached item list.
/// (The highlight of the tab buttons is handled by `update_tab_highlight`.)
pub(crate) fn handle_tab_click(
    tab_btns: Query<(&Interaction, &InventoryTabBtn), (Changed<Interaction>, With<Button>)>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    grid_query: Query<Entity, With<InventoryGrid>>,
    existing_slots: Query<Entity, With<InventorySlot>>,
    mut inv_state: ResMut<InventoryState>,
) {
    for (interaction, tab_btn) in &tab_btns {
        if *interaction != Interaction::Pressed {
            continue;
        }

        inv_state.active_tab = tab_btn.0.clone();

        let Ok(grid) = grid_query.get_single() else {
            continue;
        };
        let tab = inv_state.active_tab.clone();
        rebuild_grid(
            &mut commands,
            &asset_server,
            grid,
            &existing_slots,
            &inv_state.cached_items,
            &tab,
        );
    }
}

/// Keeps the look of the four tab buttons in sync with `InventoryState::active_tab`:
/// the active tab is highlighted and every other tab goes back to idle.
pub(crate) fn update_tab_highlight(
    inv_state: Res<InventoryState>,
    mut tabs: Query<(
        &InventoryTabBtn,
        &mut BackgroundColor,
        &mut BorderColor,
        &Children,
    )>,
    mut text_q: Query<&mut Text>,
) {
    if !inv_state.is_changed() {
        return;
    }

    for (tab, mut bg, mut border, children) in &mut tabs {
        let active = tab.0 == inv_state.active_tab;

        let bg_color: Color = if active { TAB_ACTIVE_BG } else { TAB_IDLE_BG };
        let border_color: Color = if active { BORDER_GOLD } else { Color::NONE };
        *bg = bg_color.into();
        *border = border_color.into();

        if let Some(&child) = children.first() {
            if let Ok(mut txt) = text_q.get_mut(child) {
                txt.sections[0].style.color = if active { TITLE_COLOR } else { TEXT_DIM };
            }
        }
    }
}
