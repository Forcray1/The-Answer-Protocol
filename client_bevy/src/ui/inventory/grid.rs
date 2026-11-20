use bevy::prelude::*;
use crate::AppState;
use crate::combat::CombatResource;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::LocalPlayerName;
use crate::ui::chat::ChatConsole;
use crate::ui::inventory::components::{InventoryGrid, InventorySlot, InventorySlotInfo, InventoryState, InventoryTab, InventoryUiRoot, RawInvItem};
use crate::ui::inventory::style::{BORDER_GOLD, SLOT_BG, SLOT_BORDER, TITLE_COLOR};
use crate::ui::quest::QuestState;

pub(crate) fn toggle_inventory(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<InventoryState>,
    mut query: Query<&mut Visibility, With<InventoryUiRoot>>,
    console: Res<ChatConsole>,
    quest_state: Res<QuestState>,
    sender: Res<crate::net::NetworkSender>,
    combat: Option<Res<CombatResource>>,
) {
    if let Some(ref combat) = combat {
        if combat.in_combat && state.open {
            state.open = false;
        }
        if combat.in_combat || combat.phase != crate::combat::CombatTransitionPhase::None {
            if state.is_changed() {
                if let Ok(mut visibility) = query.get_single_mut() {
                    *visibility = Visibility::Hidden;
                }
            }
            return;
        }
    }

    if !console.open
        && !console.terminal_open
        && !quest_state.open
        && keys.just_pressed(KeyCode::KeyI)
    {
        state.open = !state.open;
        if state.open {
            let _ = sender.0.send("INVENTORY\n".to_string());
        }
    }

    if state.is_changed() {
        if let Ok(mut visibility) = query.get_single_mut() {
            if state.open {
                *visibility = Visibility::Inherited;
            } else {
                *visibility = Visibility::Hidden;
            }
        }
    }
}

pub(crate) fn spawn_inv_slot(
    commands: &mut Commands,
    asset_server: &AssetServer,
    grid: Entity,
    item: &RawInvItem,
) {
    let icon: Handle<Image> = if item.sprite != "none" && !item.sprite.is_empty() {
        asset_server.load(format!("UI/{}.png", item.sprite))
    } else if item.slot == "weapon" {
        asset_server.load("UI/sword_icone.png")
    } else {
        asset_server.load("UI/QuestUI.png")
    };

    // Equipped medaillons get a gold border; others get the standard slot border.
    let border_col: Color = if item.is_equipped {
        BORDER_GOLD
    } else {
        SLOT_BORDER
    };
    let bg_col: Color = if item.is_equipped {
        Color::rgba(0.35, 0.28, 0.12, 1.0)
    } else {
        SLOT_BG
    };

    let slot_entity = commands
        .spawn((
            ButtonBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(if item.is_equipped { 3.0 } else { 1.0 })),
                    ..default()
                },
                background_color: bg_col.into(),
                border_color: border_col.into(),
                ..default()
            },
            InventorySlot,
            InventorySlotInfo {
                id: item.id.clone(),
                name: item.name.clone(),
                damage: item.damage,
                slot: item.slot.clone(),
                is_equipped: item.is_equipped,
                consumable: item.consumable,
            },
        ))
        .with_children(|slot| {
            slot.spawn(ImageBundle {
                style: Style {
                    width: Val::Px(50.0),
                    height: Val::Px(50.0),
                    ..default()
                },
                image: UiImage::new(icon),
                ..default()
            });
            if item.count > 1 {
                slot.spawn(
                    TextBundle::from_section(
                        format!("x{}", item.count),
                        TextStyle {
                            font_size: 16.0,
                            color: TITLE_COLOR,
                            ..default()
                        },
                    )
                    .with_style(Style {
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(2.0),
                        right: Val::Px(5.0),
                        ..default()
                    }),
                );
            }
            // Small "e" badge on equipped items
            if item.is_equipped {
                slot.spawn(
                    TextBundle::from_section(
                        "e",
                        TextStyle {
                            font_size: 14.0,
                            color: BORDER_GOLD,
                            ..default()
                        },
                    )
                    .with_style(Style {
                        position_type: PositionType::Absolute,
                        top: Val::Px(2.0),
                        left: Val::Px(4.0),
                        ..default()
                    }),
                );
            }
        })
        .id();
    commands.entity(grid).add_child(slot_entity);
}

pub(crate) fn rebuild_grid(
    commands: &mut Commands,
    asset_server: &AssetServer,
    grid: Entity,
    existing: &Query<Entity, With<InventorySlot>>,
    items: &[RawInvItem],
    tab: &InventoryTab,
) {
    for e in existing.iter() {
        commands.entity(e).despawn_recursive();
    }
    for item in items.iter().filter(|i| tab.matches(&i.slot, i.consumable)) {
        spawn_inv_slot(commands, asset_server, grid, item);
    }
}

pub(crate) fn handle_inventory_data(
    mut commands: Commands,
    mut events: EventReader<crate::net::ServerMessageEvent>,
    asset_server: Res<AssetServer>,
    grid_query: Query<Entity, With<InventoryGrid>>,
    existing_slots: Query<Entity, With<InventorySlot>>,
    mut inv_state: ResMut<InventoryState>,
) {
    for ev in events.read() {
        let Some(data) = ev.0.strip_prefix("S: EVT INV_DATA ") else {
            continue;
        };
        let Ok(grid) = grid_query.get_single() else {
            continue;
        };

        inv_state.cached_items.clear();

        if data.trim() != "empty" {
            for item in data.trim().split('|') {
                let parts: Vec<&str> = item.split(':').collect();
                if parts.len() >= 5 {
                    let has_sprite_field = parts.len() >= 8;
                    inv_state.cached_items.push(RawInvItem {
                        id: parts[0].to_string(),
                        count: parts[1].parse().unwrap_or(1),
                        slot: parts[2].to_string(),
                        name: parts[3].to_string(),
                        damage: parts[4].parse().unwrap_or(0),
                        sprite: if has_sprite_field {
                            parts.get(5).copied().unwrap_or("none").to_string()
                        } else {
                            "none".to_string()
                        },
                        is_equipped: if has_sprite_field {
                            parts.get(6).copied().unwrap_or("0") == "1"
                        } else {
                            parts.get(5).copied().unwrap_or("0") == "1"
                        },
                        consumable: if has_sprite_field {
                            parts.get(7).copied().unwrap_or("0") == "1"
                        } else {
                            parts.get(6).copied().unwrap_or("0") == "1"
                        },
                    });
                }
            }
        }

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
