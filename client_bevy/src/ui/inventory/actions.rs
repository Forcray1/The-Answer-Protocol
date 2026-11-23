use bevy::prelude::*;
use crate::net::NetworkSender;
use crate::ui::inventory::components::{InventoryActionBtn, InventoryActionText, InventoryDescriptionText, InventoryDropBtn, InventorySlotInfo, InventoryState, SelectedItem, UseItemBtn};
use crate::ui::inventory::style::{BORDER_GOLD, SLOT_BG, SLOT_BORDER};

// Named query types: they keep the system signatures short and readable
// (a long inline Query type triggers clippy::type_complexity).
type SlotButtonQuery<'w, 's> = Query<'w, 's, ( &'static Interaction, &'static InventorySlotInfo, &'static mut BackgroundColor, &'static mut BorderColor, ), (Changed<Interaction>, With<Button>)>;
type ActionButtonQuery<'w, 's> = Query<'w, 's, (&'static Interaction, &'static mut BackgroundColor), (Changed<Interaction>, With<InventoryActionBtn>)>;
type DropButtonQuery<'w, 's> = Query<'w, 's, (&'static Interaction, &'static mut BackgroundColor), (Changed<Interaction>, With<InventoryDropBtn>)>;
type UseItemButtonQuery<'w, 's> = Query<'w, 's, (&'static Interaction, &'static UseItemBtn), (Changed<Interaction>, With<Button>)>;

pub(crate) fn handle_slot_interactions(
    mut interaction_query: SlotButtonQuery,
    mut text_query: Query<&mut Text, With<InventoryDescriptionText>>,
    mut action_btn_q: Query<&mut Style, (With<InventoryActionBtn>, Without<InventoryDropBtn>)>,
    mut drop_btn_q: Query<&mut Style, (With<InventoryDropBtn>, Without<InventoryActionBtn>)>,
    mut action_txt_q: Query<
        &mut Text,
        (With<InventoryActionText>, Without<InventoryDescriptionText>),
    >,
    mut selected_item: ResMut<SelectedItem>,
) {
    for (interaction, info, mut color, mut border) in &mut interaction_query {
        // Equipped items keep their gold border regardless of hover state.
        let base_bg = if info.is_equipped {
            Color::rgba(0.35, 0.28, 0.12, 1.0)
        } else {
            SLOT_BG
        };
        let base_border = if info.is_equipped {
            BORDER_GOLD
        } else {
            SLOT_BORDER
        };

        match *interaction {
            Interaction::Pressed => {
                *color = Color::rgba(0.40, 0.35, 0.20, 1.0).into();
                *border = BORDER_GOLD.into();

                if let Ok(mut text) = text_query.get_single_mut() {
                    let stat_label = if info.slot == "chest" {
                        "Defense"
                    } else {
                        "Damage"
                    };
                    let stat_str = if info.damage > 0 {
                        format!(" | {}: +{}", stat_label, info.damage)
                    } else {
                        String::new()
                    };
                    text.sections[0].value = format!("{}{}", info.name, stat_str);
                }
                selected_item.id = Some(info.id.clone());
                selected_item.name = Some(info.name.clone());
                selected_item.is_equipped = info.is_equipped;
                selected_item.consumable = info.consumable;
                selected_item.slot = info.slot.clone();

                if let Ok(mut style) = action_btn_q.get_single_mut() {
                    let show = info.slot != "none" || info.consumable;
                    style.display = if show { Display::Flex } else { Display::None };
                    if let Ok(mut btn_txt) = action_txt_q.get_single_mut() {
                        btn_txt.sections[0].value = if info.consumable {
                            "Use".to_string()
                        } else if info.is_equipped {
                            "Unequip".to_string()
                        } else {
                            "Equip".to_string()
                        };
                    }
                }
                // Always show the Drop button when an item is selected
                if let Ok(mut style) = drop_btn_q.get_single_mut() {
                    style.display = Display::Flex;
                }
            }
            Interaction::Hovered => {
                *color = Color::rgba(0.32, 0.26, 0.16, 1.0).into();
                *border = BORDER_GOLD.into();
            }
            Interaction::None => {
                *color = base_bg.into();
                *border = base_border.into();
            }
        }
    }
}

/// After an inventory update, forgets a selection whose item moved or left the grid,
/// so the action buttons can't target an item that is no longer there.
pub(crate) fn clear_stale_selection(
    mut events: EventReader<crate::net::ServerMessageEvent>,
    inv_state: Res<InventoryState>,
    mut selected_item: ResMut<SelectedItem>,
    mut text_query: Query<&mut Text, With<InventoryDescriptionText>>,
    mut action_btn_q: Query<&mut Style, (With<InventoryActionBtn>, Without<InventoryDropBtn>)>,
    mut drop_btn_q: Query<&mut Style, (With<InventoryDropBtn>, Without<InventoryActionBtn>)>,
) {
    if !events.read().any(|ev| ev.0.starts_with("S: EVT INV_DATA ")) {
        return;
    }
    let Some(id) = selected_item.id.as_ref() else {
        return;
    };
    if !selected_item.is_equipped && inv_state.cached_items.iter().any(|item| &item.id == id) {
        return;
    }
    *selected_item = SelectedItem::default();
    if let Ok(mut text) = text_query.get_single_mut() {
        text.sections[0].value = "Select an item...".to_string();
    }
    if let Ok(mut style) = action_btn_q.get_single_mut() {
        style.display = Display::None;
    }
    if let Ok(mut style) = drop_btn_q.get_single_mut() {
        style.display = Display::None;
    }
}

pub(crate) fn handle_action_btn(
    mut interaction_query: ActionButtonQuery,
    selected_item: Res<SelectedItem>,
    sender: Res<crate::net::NetworkSender>,
) {
    for (interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = Color::rgba(0.50, 0.40, 0.22, 1.0).into();
                if let Some(id) = &selected_item.id {
                    if selected_item.consumable {
                        let _ = sender.0.send(format!("USE {}\n", id));
                    } else {
                        let cmd = if selected_item.is_equipped {
                            "UNEQUIP"
                        } else {
                            "EQUIP"
                        };
                        let _ = sender.0.send(format!("{} {}\n", cmd, id));
                    }
                }
            }
            Interaction::Hovered => {
                *color = Color::rgba(0.42, 0.34, 0.20, 1.0).into();
            }
            Interaction::None => {
                *color = Color::rgba(0.35, 0.28, 0.18, 1.0).into();
            }
        }
    }
}

pub(crate) fn handle_drop_btn(
    mut interaction_query: DropButtonQuery,
    selected_item: Res<SelectedItem>,
    sender: Res<crate::net::NetworkSender>,
) {
    for (interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = Color::rgba(0.55, 0.15, 0.15, 1.0).into();
                if let Some(id) = &selected_item.id {
                    let _ = sender.0.send(format!("DROP {}\n", id));
                }
            }
            Interaction::Hovered => {
                *color = Color::rgba(0.45, 0.15, 0.15, 1.0).into();
            }
            Interaction::None => {
                *color = Color::rgba(0.35, 0.12, 0.12, 1.0).into();
            }
        }
    }
}

pub(crate) fn handle_use_item_btn(
    mut btn_q: UseItemButtonQuery,
    sender: Option<Res<NetworkSender>>,
) {
    let Some(sender) = sender else { return };
    for (interaction, use_btn) in &mut btn_q {
        if *interaction == Interaction::Pressed {
            let _ = sender.0.try_send(format!("USE {}\n", use_btn.0));
        }
    }
}
