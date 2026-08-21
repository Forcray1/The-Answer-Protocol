use bevy::prelude::*;

use crate::net::{NetworkSender, ServerMessageEvent};
use crate::AppState;

const MAX_CHAT_LINES: usize = 8;

#[derive(Component)]
pub struct InventoryDescriptionText;

#[derive(Component, Clone)]
pub struct InventorySlotInfo {
    pub id: String,
    pub name: String,
    pub damage: u32,
    pub slot: String,
    pub is_equipped: bool,
}

pub struct ConsolePlugin;
pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryState>()
            .init_resource::<SelectedItem>()
            .add_systems(Startup, setup_inventory_ui)
            .add_systems(Update, (
                toggle_inventory,
                handle_inventory_data,
                handle_equip_data,
                handle_slot_interactions,
                handle_action_btn,
                handle_char_stats,
            ).run_if(in_state(AppState::InGame)));
    }
}

#[derive(Resource, Default)]
pub struct InventoryState {
    pub open: bool,
}

#[derive(Component)]
struct InventoryUiRoot;

#[derive(Component)]
struct InventoryGrid;

#[derive(Component)]
pub struct EquipmentPanel;

#[derive(Component)]
pub struct EquipmentSlot(pub String);

#[derive(Component)]
pub struct InventoryActionBtn;

#[derive(Component)]
pub struct InventoryActionText;

#[derive(Component)]
pub struct StatValueText(pub usize);

#[derive(Component)]
pub struct CloseButton(pub String);

#[derive(Resource, Default)]
pub struct SelectedItem {
    pub id: Option<String>,
    pub name: Option<String>,
    pub is_equipped: bool,
}

fn setup_inventory_ui(mut commands: Commands, _asset_server: Res<AssetServer>) {
    // ── Color palette inspired by InventoryUI.png ──
    let bg_dark: Color = Color::rgba(0.12, 0.09, 0.07, 0.97);        // Dark brown/chocolate
    let bg_panel: Color = Color::rgba(0.18, 0.14, 0.10, 0.95);       // Slightly lighter brown
    let border_gold: Color = Color::rgba(0.72, 0.58, 0.30, 1.0);     // Gold/tan border
    let border_inner: Color = Color::rgba(0.40, 0.32, 0.20, 0.8);    // Darker gold for inner borders
    let slot_bg: Color = Color::rgba(0.22, 0.18, 0.14, 1.0);         // Equipment slot background
    let slot_border: Color = Color::rgba(0.50, 0.42, 0.28, 0.7);     // Slot border color
    let title_color: Color = Color::rgba(0.90, 0.80, 0.55, 1.0);     // Gold title text
    let text_light: Color = Color::rgba(0.85, 0.82, 0.75, 1.0);      // Light parchment text
    let text_dim: Color = Color::rgba(0.55, 0.50, 0.42, 1.0);        // Dimmed placeholder text
    let stat_green: Color = Color::rgba(0.40, 0.80, 0.35, 1.0);      // Green for stat values
    let stat_red: Color = Color::rgba(0.85, 0.30, 0.25, 1.0);        // Red for damage
    let btn_bg: Color = Color::rgba(0.35, 0.28, 0.18, 1.0);          // Button background
    let btn_border: Color = Color::rgba(0.60, 0.50, 0.30, 1.0);      // Button border

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    position_type: PositionType::Absolute,
                    ..default()
                },
                visibility: Visibility::Hidden,
                z_index: ZIndex::Global(10),
                ..default()
            },
            InventoryUiRoot,
        ))
        .with_children(|parent| {
            // ── Outer frame with gold border ──
            parent.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(900.0),
                    height: Val::Px(580.0),
                    flex_direction: FlexDirection::Column,
                    border: UiRect::all(Val::Px(3.0)),
                    ..default()
                },
                background_color: bg_dark.into(),
                border_color: border_gold.into(),
                ..default()
            }).with_children(|frame| {
                // ── Title bar ──
                frame.spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Px(45.0),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        border: UiRect::bottom(Val::Px(2.0)),
                        padding: UiRect::horizontal(Val::Px(15.0)),
                        ..default()
                    },
                    background_color: Color::rgba(0.15, 0.12, 0.08, 1.0).into(),
                    border_color: border_gold.into(),
                    ..default()
                }).with_children(|title_bar| {
                    title_bar.spawn(NodeBundle { style: Style { width: Val::Px(24.0), ..default() }, ..default() });
                    title_bar.spawn(TextBundle::from_section(
                        "PERSONNAGE",
                        TextStyle { font_size: 26.0, color: title_color, ..default() },
                    ));
                    title_bar.spawn((
                        ButtonBundle {
                            style: Style {
                                width: Val::Px(24.0),
                                height: Val::Px(24.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            background_color: Color::NONE.into(),
                            ..default()
                        },
                        CloseButton("inventory".to_string()),
                    )).with_children(|btn| {
                        btn.spawn(TextBundle::from_section("X", TextStyle { font_size: 20.0, color: title_color, ..default() }));
                    });
                });

                // ── Main content area ──
                frame.spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Row,
                        padding: UiRect::all(Val::Px(15.0)),
                        column_gap: Val::Px(15.0),
                        ..default()
                    },
                    ..default()
                }).with_children(|content| {
                    // ════════════════════════════════════
                    // LEFT PANEL: Equipment + Silhouette
                    // ════════════════════════════════════
                    content.spawn(NodeBundle {
                        style: Style {
                            width: Val::Px(420.0),
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(10.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        background_color: bg_panel.into(),
                        border_color: border_inner.into(),
                        ..default()
                    }).with_children(|left| {
                        // Equipment area: left slots | center silhouette | right slots
                        left.spawn(NodeBundle {
                            style: Style {
                                width: Val::Percent(100.0),
                                flex_grow: 1.0,
                                flex_direction: FlexDirection::Row,
                                ..default()
                            },
                            ..default()
                        }).with_children(|equip_area| {
                            // Left column of equipment slots
                            equip_area.spawn((
                                NodeBundle {
                                    style: Style {
                                        width: Val::Px(75.0),
                                        height: Val::Percent(100.0),
                                        flex_direction: FlexDirection::Column,
                                        justify_content: JustifyContent::SpaceEvenly,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    ..default()
                                },
                                EquipmentPanel,
                            )).with_children(|col| {
                                // Weapon slot
                                col.spawn((
                                    ButtonBundle {
                                        style: Style {
                                            width: Val::Px(65.0),
                                            height: Val::Px(65.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border: UiRect::all(Val::Px(2.0)),
                                            ..default()
                                        },
                                        background_color: slot_bg.into(),
                                        border_color: slot_border.into(),
                                        ..default()
                                    },
                                    EquipmentSlot("weapon".to_string()),
                                )).with_children(|btn| {
                                    btn.spawn(TextBundle::from_section(
                                        "Arme",
                                        TextStyle { font_size: 12.0, color: text_dim, ..default() },
                                    ));
                                });
                                // Head slot
                                col.spawn((
                                    ButtonBundle {
                                        style: Style {
                                            width: Val::Px(65.0),
                                            height: Val::Px(65.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border: UiRect::all(Val::Px(2.0)),
                                            ..default()
                                        },
                                        background_color: slot_bg.into(),
                                        border_color: slot_border.into(),
                                        ..default()
                                    },
                                    EquipmentSlot("head".to_string()),
                                )).with_children(|btn| {
                                    btn.spawn(TextBundle::from_section(
                                        "Tete",
                                        TextStyle { font_size: 12.0, color: text_dim, ..default() },
                                    ));
                                });
                            });

                            // Center silhouette area
                            equip_area.spawn(NodeBundle {
                                style: Style {
                                    flex_grow: 1.0,
                                    height: Val::Percent(100.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                ..default()
                            }).with_children(|center| {
                                center.spawn(TextBundle::from_section(
                                    "[ Silhouette ]",
                                    TextStyle { font_size: 14.0, color: text_dim, ..default() },
                                ));
                            });

                            // Right column of equipment slots
                            equip_area.spawn(NodeBundle {
                                style: Style {
                                    width: Val::Px(75.0),
                                    height: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Column,
                                    justify_content: JustifyContent::SpaceEvenly,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                ..default()
                            }).with_children(|col| {
                                // Chest slot
                                col.spawn((
                                    ButtonBundle {
                                        style: Style {
                                            width: Val::Px(65.0),
                                            height: Val::Px(65.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border: UiRect::all(Val::Px(2.0)),
                                            ..default()
                                        },
                                        background_color: slot_bg.into(),
                                        border_color: slot_border.into(),
                                        ..default()
                                    },
                                    EquipmentSlot("chest".to_string()),
                                )).with_children(|btn| {
                                    btn.spawn(TextBundle::from_section(
                                        "Torse",
                                        TextStyle { font_size: 12.0, color: text_dim, ..default() },
                                    ));
                                });
                                // Legs slot
                                col.spawn((
                                    ButtonBundle {
                                        style: Style {
                                            width: Val::Px(65.0),
                                            height: Val::Px(65.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border: UiRect::all(Val::Px(2.0)),
                                            ..default()
                                        },
                                        background_color: slot_bg.into(),
                                        border_color: slot_border.into(),
                                        ..default()
                                    },
                                    EquipmentSlot("legs".to_string()),
                                )).with_children(|btn| {
                                    btn.spawn(TextBundle::from_section(
                                        "Jambes",
                                        TextStyle { font_size: 12.0, color: text_dim, ..default() },
                                    ));
                                });
                            });
                        });

                        // ── Action bar at bottom of left panel ──
                        left.spawn(NodeBundle {
                            style: Style {
                                width: Val::Percent(100.0),
                                height: Val::Px(50.0),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                margin: UiRect { top: Val::Px(10.0), ..default() },
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(0.0)),
                                ..default()
                            },
                            ..default()
                        }).with_children(|action_bar| {
                            // Description text
                            action_bar.spawn((
                                TextBundle::from_section(
                                    "Selectionnez un objet...",
                                    TextStyle { font_size: 13.0, color: text_light, ..default() },
                                ).with_style(Style {
                                    max_width: Val::Px(250.0),
                                    ..default()
                                }),
                                InventoryDescriptionText,
                            ));

                            // Equip/Unequip button
                            action_bar.spawn((
                                ButtonBundle {
                                    style: Style {
                                        width: Val::Px(120.0),
                                        height: Val::Px(35.0),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        display: Display::None,
                                        border: UiRect::all(Val::Px(2.0)),
                                        ..default()
                                    },
                                    background_color: btn_bg.into(),
                                    border_color: btn_border.into(),
                                    ..default()
                                },
                                InventoryActionBtn,
                            )).with_children(|btn| {
                                btn.spawn((
                                    TextBundle::from_section(
                                        "Equiper",
                                        TextStyle { font_size: 14.0, color: title_color, ..default() },
                                    ),
                                    InventoryActionText,
                                ));
                            });
                        });
                    });

                    // ════════════════════════════════════
                    // RIGHT PANEL: Stats + Inventory Grid
                    // ════════════════════════════════════
                    content.spawn(NodeBundle {
                        style: Style {
                            flex_grow: 1.0,
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(10.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            row_gap: Val::Px(10.0),
                            ..default()
                        },
                        background_color: bg_panel.into(),
                        border_color: border_inner.into(),
                        ..default()
                    }).with_children(|right| {
                        // ── Stats section ──
                        right.spawn(NodeBundle {
                            style: Style {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                padding: UiRect::all(Val::Px(8.0)),
                                row_gap: Val::Px(6.0),
                                border: UiRect::bottom(Val::Px(1.0)),
                                ..default()
                            },
                            border_color: border_inner.into(),
                            ..default()
                        }).with_children(|stats_section| {
                            // Stat rows with indexed markers
                            let stat_rows: Vec<(&str, Color, usize)> = vec![
                                ("Degats", stat_red, 0),
                                ("Degats Spe.", stat_red, 1),
                                ("Defense", stat_green, 2),
                                ("Defense Spe.", stat_green, 3),
                                ("Vie", stat_green, 4),
                            ];
                            for (label, color, idx) in stat_rows {
                                stats_section.spawn(NodeBundle {
                                    style: Style {
                                        width: Val::Percent(100.0),
                                        flex_direction: FlexDirection::Row,
                                        justify_content: JustifyContent::SpaceBetween,
                                        ..default()
                                    },
                                    ..default()
                                }).with_children(|row| {
                                    row.spawn(TextBundle::from_section(
                                        format!("  {}", label),
                                        TextStyle { font_size: 15.0, color: text_light, ..default() },
                                    ));
                                    row.spawn((
                                        TextBundle::from_section(
                                            "0",
                                            TextStyle { font_size: 15.0, color, ..default() },
                                        ),
                                        StatValueText(idx),
                                    ));
                                });
                            }
                        });

                        // ── Inventory grid ──
                        right.spawn((
                            NodeBundle {
                                style: Style {
                                    flex_grow: 1.0,
                                    display: Display::Grid,
                                    grid_template_columns: vec![GridTrack::flex(1.0); 5],
                                    grid_template_rows: vec![GridTrack::flex(1.0); 4],
                                    row_gap: Val::Px(6.0),
                                    column_gap: Val::Px(6.0),
                                    ..default()
                                },
                                ..default()
                            },
                            InventoryGrid,
                        ));
                    });
                });
            });
        });
}

fn toggle_inventory(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<InventoryState>,
    mut query: Query<&mut Visibility, With<InventoryUiRoot>>,
    console: Res<ChatConsole>,
    quest_state: Res<QuestState>,
    sender: Res<crate::net::NetworkSender>,
) {
    if !console.open && !quest_state.open && keys.just_pressed(KeyCode::KeyI) {
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

#[derive(Component)]
struct InventorySlot;

fn handle_inventory_data(
    mut commands: Commands,
    mut events: EventReader<crate::net::ServerMessageEvent>,
    asset_server: Res<AssetServer>,
    grid_query: Query<Entity, With<InventoryGrid>>,
    existing_slots: Query<Entity, With<InventorySlot>>,
) {
    for ev in events.read() {
        if let Some(data) = ev.0.strip_prefix("S: EVT INV_DATA ") {
            let Ok(grid) = grid_query.get_single() else { continue };

            // Remove old slots
            for entity in existing_slots.iter() {
                commands.entity(entity).despawn_recursive();
            }

            if data.trim() == "empty" {
                continue;
            }

            for item in data.trim().split('|') {
                let parts: Vec<&str> = item.split(':').collect();
                if parts.len() >= 5 {
                    let count: u32 = parts[1].parse().unwrap_or(1);
                    let slot_type = parts[2];
                    let item_name = parts[3].to_string();
                    let damage: u32 = parts[4].parse().unwrap_or(0);
                    let sprite_name = if parts.len() > 5 { parts[5] } else { "none" };
                    
                    let mut icon = None;
                    if sprite_name != "none" && !sprite_name.is_empty() {
                        icon = Some(asset_server.load(format!("UI/{}.png", sprite_name)));
                    } else if slot_type == "weapon" {
                        icon = Some(asset_server.load("UI/sword_icone.png"));
                    }
                    
                    if let Some(texture) = icon {
                        let slot_entity = commands.spawn((
                            ButtonBundle {
                                style: Style {
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                background_color: Color::rgba(0.22, 0.18, 0.14, 1.0).into(),
                                border_color: Color::rgba(0.50, 0.42, 0.28, 0.7).into(),
                                ..default()
                            },
                            InventorySlot,
                            InventorySlotInfo {
                                id: parts[0].to_string(),
                                name: item_name,
                                damage,
                                slot: slot_type.to_string(),
                                is_equipped: false,
                            },
                        )).with_children(|slot| {
                            slot.spawn(ImageBundle {
                                style: Style {
                                    width: Val::Px(50.0),
                                    height: Val::Px(50.0),
                                    ..default()
                                },
                                image: UiImage::new(texture),
                                ..default()
                            });
                            
                            if count > 1 {
                                slot.spawn(TextBundle::from_section(
                                    format!("x{}", count),
                                    TextStyle { font_size: 16.0, color: Color::rgba(0.90, 0.80, 0.55, 1.0), ..default() },
                                ).with_style(Style {
                                    position_type: PositionType::Absolute,
                                    bottom: Val::Px(2.0),
                                    right: Val::Px(5.0),
                                    ..default()
                                }));
                            }
                        }).id();
                        commands.entity(grid).add_child(slot_entity);
                    }
                }
            }
        }
    }
}

fn handle_slot_interactions(
    mut interaction_query: Query<
        (&Interaction, &InventorySlotInfo, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut text_query: Query<&mut Text, With<InventoryDescriptionText>>,
    mut action_btn_q: Query<&mut Style, With<InventoryActionBtn>>,
    mut action_txt_q: Query<&mut Text, (With<InventoryActionText>, Without<InventoryDescriptionText>)>,
    mut selected_item: ResMut<SelectedItem>,
) {
    for (interaction, info, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = Color::rgba(0.40, 0.35, 0.25, 1.0).into(); // Gold highlight
                if let Ok(mut text) = text_query.get_single_mut() {
                    let dmg_str = if info.damage > 0 { format!(" | Degats: +{}", info.damage) } else { "".to_string() };
                    text.sections[0].value = format!("{} {}", info.name, dmg_str);
                }
                
                selected_item.id = Some(info.id.clone());
                selected_item.name = Some(info.name.clone());
                selected_item.is_equipped = info.is_equipped;
                
                if let Ok(mut style) = action_btn_q.get_single_mut() {
                    if info.slot != "none" {
                        style.display = Display::Flex;
                        if let Ok(mut btn_txt) = action_txt_q.get_single_mut() {
                            btn_txt.sections[0].value = if info.is_equipped { "Desequiper".to_string() } else { "Equiper".to_string() };
                        }
                    } else {
                        style.display = Display::None;
                    }
                }
            }
            Interaction::Hovered => {
                *color = Color::rgba(0.30, 0.25, 0.18, 1.0).into(); // Warm brown hover
            }
            Interaction::None => {
                *color = Color::rgba(0.22, 0.18, 0.14, 1.0).into(); // Default slot color
            }
        }
    }
}

fn handle_action_btn(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<InventoryActionBtn>),
    >,
    selected_item: Res<SelectedItem>,
    sender: Res<crate::net::NetworkSender>,
) {
    for (interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = Color::rgba(0.50, 0.40, 0.22, 1.0).into();
                if let Some(id) = &selected_item.id {
                    let cmd = if selected_item.is_equipped { "UNEQUIP" } else { "EQUIP" };
                    let _ = sender.0.send(format!("{} {}\n", cmd, id));
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

fn handle_equip_data(
    mut commands: Commands,
    mut events: EventReader<crate::net::ServerMessageEvent>,
    asset_server: Res<AssetServer>,
    mut slots_query: Query<(Entity, &EquipmentSlot)>,
) {
    for ev in events.read() {
        if let Some(data) = ev.0.strip_prefix("S: EVT EQUIP_DATA ") {
            for item in data.trim().split('|') {
                let parts: Vec<&str> = item.split(':').collect();
                if parts.len() >= 2 {
                    let slot_name = parts[0];
                    let id = parts[1];
                    
                    let mut is_equipped = false;
                    let mut item_name = "Vide".to_string();
                    let mut damage = 0;
                    
                    let mut sprite_name = "none";
                    if id != "none" && parts.len() >= 4 {
                        is_equipped = true;
                        item_name = parts[2].to_string();
                        damage = parts[3].parse().unwrap_or(0);
                        if parts.len() >= 5 {
                            sprite_name = parts[4];
                        }
                    }
                    
                    for (entity, slot_cmp) in slots_query.iter_mut() {
                        if slot_cmp.0 == slot_name {
                            // Update slot UI
                            commands.entity(entity).despawn_descendants();
                            if is_equipped {
                                commands.entity(entity).insert(InventorySlotInfo {
                                    id: id.to_string(),
                                    name: item_name.clone(),
                                    damage,
                                    slot: slot_name.to_string(),
                                    is_equipped: true,
                                });
                                
                                let mut icon = None;
                                if sprite_name != "none" && !sprite_name.is_empty() {
                                    icon = Some(asset_server.load(format!("UI/{}.png", sprite_name)));
                                } else if slot_name == "weapon" {
                                    icon = Some(asset_server.load("UI/sword_icone.png"));
                                }
                                
                                if let Some(texture) = icon {
                                    commands.entity(entity).with_children(|p| {
                                        p.spawn(ImageBundle {
                                            style: Style {
                                                width: Val::Px(50.0),
                                                height: Val::Px(50.0),
                                                ..default()
                                            },
                                            image: UiImage::new(texture),
                                            ..default()
                                        });
                                    });
                                } else {
                                    commands.entity(entity).with_children(|p| {
                                        p.spawn(TextBundle::from_section(
                                            &item_name,
                                            TextStyle { font_size: 12.0, color: Color::rgba(0.85, 0.82, 0.75, 1.0), ..default() },
                                        ));
                                    });
                                }
                            } else {
                                commands.entity(entity).remove::<InventorySlotInfo>();
                                commands.entity(entity).with_children(|p| {
                                    let label = match slot_name {
                                        "head" => "Casque",
                                        "chest" => "Torse",
                                        "legs" => "Jambes",
                                        "weapon" => "Arme",
                                        _ => "Vide",
                                    };
                                    p.spawn(TextBundle::from_section(
                                        label,
                                        TextStyle { font_size: 12.0, color: Color::rgba(0.55, 0.50, 0.42, 1.0), ..default() },
                                    ));
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

fn handle_char_stats(
    mut events: EventReader<crate::net::ServerMessageEvent>,
    mut stat_texts: Query<(&mut Text, &StatValueText)>,
) {
    for ev in events.read() {
        if let Some(data) = ev.0.strip_prefix("S: EVT CHAR_STATS ") {
            let parts: Vec<&str> = data.trim().split_whitespace().collect();
            if parts.len() >= 5 {
                let values: Vec<i32> = parts.iter().filter_map(|p| p.parse().ok()).collect();
                if values.len() >= 5 {
                    // 0=damage, 1=spe_damage, 2=defense, 3=spe_defense, 4=max_hp
                    for (mut text, stat_marker) in stat_texts.iter_mut() {
                        if stat_marker.0 < values.len() {
                            text.sections[0].value = values[stat_marker.0].to_string();
                        }
                    }
                }
            }
        }
    }
}

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatConsole>()
            .add_systems(Startup, setup_ui)
            .add_systems(
                Update,
                (
                    toggle_chat,
                    handle_inputs.after(toggle_chat),
                    display_messages,
                    spawn_chat_bubbles,
                    tick_chat_bubbles,
                ).run_if(in_state(AppState::InGame)),
            );
    }
}

#[derive(Resource, Default)]
pub struct ChatConsole {
    pub open: bool,
    just_opened: bool,
}

#[derive(Component)]
struct ChatUiRoot;

#[derive(Component)]
struct ChatText;

#[derive(Component)]
struct InputText;

fn setup_ui(mut commands: Commands) {
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::FlexEnd,
                    align_items: AlignItems::FlexStart,
                    ..default()
                },
                visibility: Visibility::Hidden,
                ..default()
            },
            ChatUiRoot,
        ))
        .with_children(|parent| {
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Px(600.0),
                        height: Val::Px(250.0),
                        margin: UiRect { left: Val::Px(15.0), bottom: Val::Px(5.0), ..default() },
                        padding: UiRect::all(Val::Px(15.0)),
                        flex_direction: FlexDirection::ColumnReverse,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    background_color: Color::rgba(0.0, 0.0, 0.0, 0.8).into(),
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn((
                        TextBundle::from_section(
                            "Connecting...\n",
                            TextStyle { font_size: 20.0, color: Color::WHITE, ..default() },
                        ),
                        ChatText,
                    ));
                });
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Px(600.0),
                        height: Val::Px(40.0),
                        margin: UiRect { left: Val::Px(15.0), bottom: Val::Px(15.0), ..default() },
                        padding: UiRect { left: Val::Px(15.0), right: Val::Px(15.0), top: Val::Px(8.0), bottom: Val::Px(8.0) },
                        ..default()
                    },
                    background_color: Color::rgba(0.1, 0.1, 0.1, 0.9).into(),
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn((
                        TextBundle::from_section(
                            "> ",
                            TextStyle { font_size: 20.0, color: Color::YELLOW, ..default() },
                        ),
                        InputText,
                    ));
                });
        });
}

fn toggle_chat(
    keys: Res<ButtonInput<KeyCode>>,
    mut console: ResMut<ChatConsole>,
    mut query: Query<&mut Visibility, With<ChatUiRoot>>,
) {
    let new_state = if !console.open && keys.just_pressed(KeyCode::KeyT) {
        Some(true)
    } else if console.open && keys.just_pressed(KeyCode::Escape) {
        Some(false)
    } else {
        None
    };

    if let Some(open) = new_state {
        console.open = open;
        console.just_opened = open;
        if let Ok(mut visibility) = query.get_single_mut() {
            *visibility = if open { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
}

fn handle_inputs(
    mut char_evr: EventReader<ReceivedCharacter>,
    keys: Res<ButtonInput<KeyCode>>,
    sender: Res<NetworkSender>,
    mut console: ResMut<ChatConsole>,
    mut query: Query<&mut Text, With<InputText>>,
) {
    if !console.open {
        return;
    }
    let swallow_chars = std::mem::take(&mut console.just_opened);

    let Ok(mut text) = query.get_single_mut() else {
        return;
    };

    for ev in char_evr.read() {
        if swallow_chars {
            continue;
        }
        let s = ev.char.to_string();
        if !s.contains('\u{8}') && !s.contains('\r') && !s.contains('\n') {
            text.sections[0].value.push_str(&s);
        }
    }

    if keys.just_pressed(KeyCode::Backspace) && text.sections[0].value.chars().count() > 2 {
        text.sections[0].value.pop();
    }

    if keys.just_pressed(KeyCode::Enter) {
        let command = text.sections[0].value[2..].trim().to_string();
        if !command.is_empty() {
            let _ = sender.0.send(format!("{}\n", command));
            text.sections[0].value = "> ".to_string();
        }
    }
}

fn display_messages(mut events: EventReader<ServerMessageEvent>, mut query: Query<&mut Text, With<ChatText>>) {
    for ev in events.read() {
        for mut text in query.iter_mut() {
            text.sections[0].value.push_str(&format!("{}\n", ev.0));

            let lines: Vec<&str> = text.sections[0].value.lines().collect();
            if lines.len() > MAX_CHAT_LINES {
                text.sections[0].value = lines[lines.len() - MAX_CHAT_LINES..].join("\n") + "\n";
            }
        }
    }
}

#[derive(Component)]
pub struct ChatBubble {
    timer: Timer,
}

fn spawn_chat_bubbles(
    mut commands: Commands,
    mut events: EventReader<ServerMessageEvent>,
    players: Query<(Entity, &crate::player::PlayerName)>,
    mut existing_bubbles: Query<(&Parent, &mut ChatBubble, &mut Sprite, Option<&Children>)>,
    mut texts: Query<&mut Text>,
) {
    for ev in events.read() {
        let parts: Vec<&str> = ev.0.split_whitespace().collect();
        // S: EVT ROOM <room> CHAT <user> <msg>
        if parts.len() >= 7 && parts[0] == "S:" && parts[1] == "EVT" && parts[2] == "ROOM" && parts[4] == "CHAT" {
            let username = parts[5];
            let message = parts[6..].join(" ");
            let text_len = message.chars().count() as f32;
            let bubble_width = (text_len * 14.0).max(50.0);
            
            for (player_ent, name) in &players {
                if name.0 == username {
                    let mut found = false;
                    for (parent, mut bubble, mut sprite, children) in &mut existing_bubbles {
                        if parent.get() == player_ent {
                            // Update existing background size
                            sprite.custom_size = Some(Vec2::new(bubble_width + 20.0, 40.0));
                            bubble.timer.reset();
                            
                            // Update existing text
                            if let Some(children) = children {
                                for &child in children.iter() {
                                    if let Ok(mut text) = texts.get_mut(child) {
                                        text.sections[0].value = message.clone();
                                    }
                                }
                            }
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        commands.entity(player_ent).with_children(|p| {
                            let text_len = message.chars().count() as f32;
                            let bubble_width = (text_len * 14.0).max(50.0);
                            
                            p.spawn((
                                SpriteBundle {
                                    sprite: Sprite {
                                        color: Color::WHITE,
                                        custom_size: Some(Vec2::new(bubble_width + 20.0, 40.0)),
                                        ..default()
                                    },
                                    transform: Transform::from_xyz(0.0, 130.0, 600.0),
                                    ..default()
                                },
                                ChatBubble {
                                    timer: Timer::from_seconds(5.0, TimerMode::Once),
                                }
                            ))
                            .with_children(|bubble_parent| {
                                bubble_parent.spawn(Text2dBundle {
                                    text: Text::from_section(
                                        message.clone(),
                                        TextStyle { font_size: 24.0, color: Color::BLACK, ..default() },
                                    ).with_justify(JustifyText::Center),
                                    transform: Transform::from_xyz(0.0, 0.0, 1.0),
                                    ..default()
                                });
                            });
                        });
                    }
                }
            }
        }
    }
}

fn tick_chat_bubbles(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut ChatBubble)>,
) {
    for (entity, mut bubble) in &mut query {
        bubble.timer.tick(time.delta());
        if bubble.timer.just_finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Quest Journal UI  (touche U)
// ──────────────────────────────────────────────────────────────────────────────

pub struct QuestPlugin;

impl Plugin for QuestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<QuestState>()
            .init_resource::<SelectedQuest>()
            .add_systems(Startup, setup_quest_ui)
            .add_systems(Update, (
                toggle_quest_ui,
                handle_quest_data,
                handle_quest_selection,
                handle_close_buttons,
            ).run_if(in_state(AppState::InGame)));
    }
}

fn handle_close_buttons(
    mut interaction_query: Query<(&Interaction, &CloseButton), (Changed<Interaction>, With<Button>)>,
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

#[derive(Resource, Default)]
pub struct QuestState {
    pub open: bool,
}

#[derive(Resource, Default)]
struct SelectedQuest {
    id: Option<String>,
}

#[derive(Component)]
struct QuestUiRoot;

#[derive(Component)]
struct QuestListPanel;

#[derive(Component)]
struct QuestEmptyText;

#[derive(Component)]
struct QuestDetailTitle;

#[derive(Component)]
struct QuestDetailDescription;

#[derive(Component)]
struct QuestDetailObjective;

#[derive(Component)]
struct QuestEntry {
    id: String,
    name: String,
    description: String,
    objective: String,
}

#[derive(Component)]
struct QuestEntrySlot;

// Couleurs du thème "Quest Journal"
const QUEST_BG: Color          = Color::rgba(0.08, 0.07, 0.06, 0.96);
const QUEST_BORDER: Color      = Color::rgba(0.55, 0.42, 0.18, 1.0);
const QUEST_TITLE_COLOR: Color = Color::rgba(0.90, 0.75, 0.35, 1.0);
const QUEST_ENTRY_BG: Color    = Color::rgba(0.30, 0.24, 0.10, 0.92);
const QUEST_ENTRY_BORDER: Color = Color::rgba(0.50, 0.40, 0.15, 0.8);
const QUEST_ENTRY_TEXT: Color  = Color::rgba(0.88, 0.76, 0.42, 1.0);
const QUEST_PARCHMENT: Color   = Color::rgba(0.82, 0.75, 0.60, 0.92);
const QUEST_DETAIL_TITLE: Color = Color::rgba(0.15, 0.12, 0.08, 1.0);
const QUEST_DETAIL_TEXT: Color  = Color::rgba(0.25, 0.22, 0.18, 1.0);
const QUEST_ENTRY_HOVER: Color  = Color::rgba(0.40, 0.33, 0.14, 0.95);
const QUEST_ENTRY_SELECTED: Color = Color::rgba(0.48, 0.38, 0.16, 1.0);

fn setup_quest_ui(mut commands: Commands) {
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    position_type: PositionType::Absolute,
                    ..default()
                },
                visibility: Visibility::Hidden,
                z_index: ZIndex::Global(10),
                ..default()
            },
            QuestUiRoot,
        ))
        .with_children(|root| {
            // ── Main Window ──
            root.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(780.0),
                    height: Val::Px(520.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(16.0)),
                    border: UiRect::all(Val::Px(3.0)),
                    ..default()
                },
                background_color: QUEST_BG.into(),
                border_color: QUEST_BORDER.into(),
                ..default()
            })
            .with_children(|window| {
                // ── Header ──
                window.spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        padding: UiRect::new(Val::Px(12.0), Val::Px(12.0), Val::Px(8.0), Val::Px(12.0)),
                        border: UiRect::bottom(Val::Px(2.0)),
                        margin: UiRect::bottom(Val::Px(12.0)),
                        ..default()
                    },
                    border_color: QUEST_BORDER.into(),
                    ..default()
                })
                .with_children(|header| {
                    header.spawn(NodeBundle { style: Style { width: Val::Px(24.0), ..default() }, ..default() });
                    header.spawn(TextBundle::from_section(
                        "Journal de Quetes",
                        TextStyle { font_size: 28.0, color: QUEST_TITLE_COLOR, ..default() },
                    ));
                    header.spawn((
                        ButtonBundle {
                            style: Style {
                                width: Val::Px(24.0),
                                height: Val::Px(24.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            background_color: Color::NONE.into(),
                            ..default()
                        },
                        CloseButton("quests".to_string()),
                    )).with_children(|btn| {
                        btn.spawn(TextBundle::from_section("X", TextStyle { font_size: 24.0, color: QUEST_TITLE_COLOR, ..default() }));
                    });
                });

                // ── Split Panel ──
                window.spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(12.0),
                        ..default()
                    },
                    ..default()
                })
                .with_children(|split| {
                    // ── Left Panel: Quest List ──
                    split.spawn(NodeBundle {
                        style: Style {
                            width: Val::Percent(38.0),
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            overflow: Overflow::clip_y(),
                            row_gap: Val::Px(6.0),
                            padding: UiRect::all(Val::Px(6.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        border_color: Color::rgba(0.4, 0.35, 0.2, 0.5).into(),
                        ..default()
                    })
                    .with_children(|left| {
                        // Quest list container
                        left.spawn((
                            NodeBundle {
                                style: Style {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(6.0),
                                    ..default()
                                },
                                ..default()
                            },
                            QuestListPanel,
                        ));

                        // Empty state message
                        left.spawn((
                            TextBundle::from_section(
                                "Aucune quete active",
                                TextStyle { font_size: 18.0, color: Color::rgba(0.6, 0.55, 0.4, 0.7), ..default() },
                            ).with_style(Style {
                                margin: UiRect::top(Val::Px(20.0)),
                                align_self: AlignSelf::Center,
                                ..default()
                            }),
                            QuestEmptyText,
                        ));
                    });

                    // ── Right Panel: Quest Details (parchment) ──
                    split.spawn(NodeBundle {
                        style: Style {
                            flex_grow: 1.0,
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(20.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            row_gap: Val::Px(16.0),
                            ..default()
                        },
                        background_color: QUEST_PARCHMENT.into(),
                        border_color: Color::rgba(0.6, 0.5, 0.3, 0.6).into(),
                        ..default()
                    })
                    .with_children(|right| {
                        // Quest Title
                        right.spawn((
                            TextBundle::from_section(
                                "Selectionnez une quete",
                                TextStyle { font_size: 24.0, color: QUEST_DETAIL_TITLE, ..default() },
                            ),
                            QuestDetailTitle,
                        ));

                        // Separator
                        right.spawn(NodeBundle {
                            style: Style {
                                width: Val::Percent(100.0),
                                height: Val::Px(2.0),
                                ..default()
                            },
                            background_color: Color::rgba(0.5, 0.4, 0.25, 0.5).into(),
                            ..default()
                        });

                        // Description label + text
                        right.spawn(NodeBundle {
                            style: Style {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(4.0),
                                ..default()
                            },
                            ..default()
                        })
                        .with_children(|desc_block| {
                            desc_block.spawn(TextBundle::from_section(
                                "Description",
                                TextStyle { font_size: 16.0, color: Color::rgba(0.4, 0.35, 0.25, 0.8), ..default() },
                            ));
                            desc_block.spawn((
                                TextBundle::from_section(
                                    "",
                                    TextStyle { font_size: 18.0, color: QUEST_DETAIL_TEXT, ..default() },
                                ),
                                QuestDetailDescription,
                            ));
                        });

                        // Objective label + text
                        right.spawn(NodeBundle {
                            style: Style {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(4.0),
                                ..default()
                            },
                            ..default()
                        })
                        .with_children(|obj_block| {
                            obj_block.spawn(TextBundle::from_section(
                                "Objectif",
                                TextStyle { font_size: 16.0, color: Color::rgba(0.4, 0.35, 0.25, 0.8), ..default() },
                            ));
                            obj_block.spawn((
                                TextBundle::from_section(
                                    "",
                                    TextStyle { font_size: 18.0, color: Color::rgba(0.45, 0.30, 0.12, 1.0), ..default() },
                                ),
                                QuestDetailObjective,
                            ));
                        });
                    });
                });
            });
        });
}

fn toggle_quest_ui(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<QuestState>,
    mut query: Query<&mut Visibility, With<QuestUiRoot>>,
    console: Res<ChatConsole>,
    inventory: Res<InventoryState>,
    sender: Res<crate::net::NetworkSender>,
) {
    if !console.open && !inventory.open && keys.just_pressed(KeyCode::KeyU) {
        state.open = !state.open;
        if state.open {
            let _ = sender.0.send("QUESTS\n".to_string());
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

fn handle_quest_data(
    mut commands: Commands,
    mut events: EventReader<crate::net::ServerMessageEvent>,
    list_query: Query<Entity, With<QuestListPanel>>,
    existing_entries: Query<Entity, With<QuestEntrySlot>>,
    mut empty_text: Query<&mut Style, With<QuestEmptyText>>,
) {
    for ev in events.read() {
        if let Some(data) = ev.0.strip_prefix("S: EVT QUEST_DATA ") {
            let Ok(list) = list_query.get_single() else { continue };

            // Remove old entries
            for entity in existing_entries.iter() {
                commands.entity(entity).despawn_recursive();
            }

            if data.trim() == "empty" {
                // Show "Aucune quête active"
                if let Ok(mut style) = empty_text.get_single_mut() {
                    style.display = Display::Flex;
                }
                continue;
            }

            // Hide empty text
            if let Ok(mut style) = empty_text.get_single_mut() {
                style.display = Display::None;
            }

            for item in data.trim().split('|') {
                let parts: Vec<&str> = item.splitn(4, ':').collect();
                if parts.len() >= 4 {
                    let quest_id = parts[0].to_string();
                    let quest_name = parts[1].to_string();
                    let quest_desc = parts[2].to_string();
                    let quest_obj = parts[3].to_string();

                    let entry_entity = commands.spawn((
                        ButtonBundle {
                            style: Style {
                                width: Val::Percent(100.0),
                                min_height: Val::Px(44.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                padding: UiRect::new(Val::Px(12.0), Val::Px(12.0), Val::Px(8.0), Val::Px(8.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            background_color: QUEST_ENTRY_BG.into(),
                            border_color: QUEST_ENTRY_BORDER.into(),
                            ..default()
                        },
                        QuestEntrySlot,
                        QuestEntry {
                            id: quest_id,
                            name: quest_name.clone(),
                            description: quest_desc,
                            objective: quest_obj,
                        },
                    )).with_children(|btn| {
                        btn.spawn(TextBundle::from_section(
                            quest_name,
                            TextStyle { font_size: 17.0, color: QUEST_ENTRY_TEXT, ..default() },
                        ));
                    }).id();
                    commands.entity(list).add_child(entry_entity);
                }
            }
        }
    }
}

fn handle_quest_selection(
    mut interaction_query: Query<
        (&Interaction, &QuestEntry, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>, With<QuestEntrySlot>),
    >,
    mut title_q: Query<&mut Text, With<QuestDetailTitle>>,
    mut desc_q: Query<&mut Text, (With<QuestDetailDescription>, Without<QuestDetailTitle>, Without<QuestDetailObjective>)>,
    mut obj_q: Query<&mut Text, (With<QuestDetailObjective>, Without<QuestDetailTitle>, Without<QuestDetailDescription>)>,
    mut selected: ResMut<SelectedQuest>,
) {
    for (interaction, entry, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = QUEST_ENTRY_SELECTED.into();
                selected.id = Some(entry.id.clone());

                if let Ok(mut title) = title_q.get_single_mut() {
                    title.sections[0].value = entry.name.clone();
                }
                if let Ok(mut desc) = desc_q.get_single_mut() {
                    desc.sections[0].value = entry.description.clone();
                }
                if let Ok(mut obj) = obj_q.get_single_mut() {
                    obj.sections[0].value = entry.objective.clone();
                }
            }
            Interaction::Hovered => {
                if selected.id.as_deref() != Some(&entry.id) {
                    *color = QUEST_ENTRY_HOVER.into();
                }
            }
            Interaction::None => {
                if selected.id.as_deref() != Some(&entry.id) {
                    *color = QUEST_ENTRY_BG.into();
                }
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Player HUD (HP & XP)
// ──────────────────────────────────────────────────────────────────────────────

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerStats>()
            .add_systems(Startup, setup_hud_ui)
            .add_systems(Update, handle_player_stats.run_if(in_state(AppState::InGame)))
            .add_systems(OnEnter(AppState::InGame), show_hud)
            .add_systems(OnExit(AppState::InGame), hide_hud);
    }
}

#[derive(Resource, Default)]
pub struct PlayerStats {
    pub hp: i32,
    pub max_hp: i32,
    pub xp: i32,
    pub max_xp: i32,
    pub level: i32,
}

#[derive(Component)]
struct HudUiRoot;

#[derive(Component)]
struct HpBarFill;

#[derive(Component)]
struct HpText;

#[derive(Component)]
struct XpBarFill;

#[derive(Component)]
struct XpText;

fn setup_hud_ui(mut commands: Commands) {
    // HUD Root - Top Left
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    top: Val::Px(20.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                visibility: Visibility::Hidden,
                z_index: ZIndex::Global(5),
                ..default()
            },
            HudUiRoot,
        ))
        .with_children(|root| {
            // ── HP Bar ──
            root.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(250.0),
                    height: Val::Px(25.0),
                    padding: UiRect::all(Val::Px(2.0)), // Inset for the fill
                    ..default()
                },
                background_color: Color::rgba(0.1, 0.1, 0.1, 0.8).into(),
                ..default()
            })
            .with_children(|bg| {
                // Fill
                bg.spawn((
                    NodeBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        background_color: Color::rgba(0.8, 0.2, 0.2, 0.9).into(), // Red
                        ..default()
                    },
                    HpBarFill,
                ));
                // Text overlay
                bg.spawn((
                    TextBundle::from_section(
                        "100/100",
                        TextStyle {
                            font_size: 16.0,
                            color: Color::WHITE,
                            ..default()
                        },
                    )
                    .with_style(Style {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(50.0),
                        top: Val::Percent(50.0),
                        margin: UiRect {
                            left: Val::Px(-30.0),
                            top: Val::Px(-8.0),
                            ..default()
                        },
                        ..default()
                    }),
                    HpText,
                ));
            });

            // ── XP Bar ──
            root.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(250.0),
                    height: Val::Px(25.0),
                    padding: UiRect::all(Val::Px(2.0)), // Inset for the fill
                    ..default()
                },
                background_color: Color::rgba(0.1, 0.1, 0.1, 0.8).into(),
                ..default()
            })
            .with_children(|bg| {
                // Fill
                bg.spawn((
                    NodeBundle {
                        style: Style {
                            width: Val::Percent(0.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        background_color: Color::rgba(0.2, 0.6, 0.8, 0.9).into(), // Blue
                        ..default()
                    },
                    XpBarFill,
                ));
                // Text overlay
                bg.spawn((
                    TextBundle::from_section(
                        "Niv 1  0/100",
                        TextStyle {
                            font_size: 16.0,
                            color: Color::WHITE,
                            ..default()
                        },
                    )
                    .with_style(Style {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(50.0),
                        top: Val::Percent(50.0),
                        margin: UiRect {
                            left: Val::Px(-40.0),
                            top: Val::Px(-8.0),
                            ..default()
                        },
                        ..default()
                    }),
                    XpText,
                ));
            });
        });
}

fn show_hud(mut q: Query<&mut Visibility, With<HudUiRoot>>) {
    for mut vis in q.iter_mut() {
        *vis = Visibility::Inherited;
    }
}

fn hide_hud(mut q: Query<&mut Visibility, With<HudUiRoot>>) {
    for mut vis in q.iter_mut() {
        *vis = Visibility::Hidden;
    }
}

fn handle_player_stats(
    mut events: EventReader<crate::net::ServerMessageEvent>,
    mut stats: ResMut<PlayerStats>,
    mut hp_fill_q: Query<&mut Style, (With<HpBarFill>, Without<XpBarFill>)>,
    mut hp_text_q: Query<&mut Text, (With<HpText>, Without<XpText>)>,
    mut xp_fill_q: Query<&mut Style, (With<XpBarFill>, Without<HpBarFill>)>,
    mut xp_text_q: Query<&mut Text, (With<XpText>, Without<HpText>)>,
) {
    for ev in events.read() {
        if let Some(data) = ev.0.strip_prefix("S: EVT PLAYER_STATS ") {
            let parts: Vec<&str> = data.trim().split_whitespace().collect();
            if parts.len() >= 5 {
                if let (Ok(hp), Ok(max_hp), Ok(xp), Ok(max_xp), Ok(lvl)) = (
                    parts[0].parse::<i32>(),
                    parts[1].parse::<i32>(),
                    parts[2].parse::<i32>(),
                    parts[3].parse::<i32>(),
                    parts[4].parse::<i32>(),
                ) {
                    stats.hp = hp;
                    stats.max_hp = max_hp;
                    stats.xp = xp;
                    stats.max_xp = max_xp;
                    stats.level = lvl;

                    // Update HP UI
                    let hp_pct = if max_hp > 0 { (hp as f32 / max_hp as f32).clamp(0.0, 1.0) * 100.0 } else { 0.0 };
                    if let Ok(mut style) = hp_fill_q.get_single_mut() {
                        style.width = Val::Percent(hp_pct);
                    }
                    if let Ok(mut text) = hp_text_q.get_single_mut() {
                        text.sections[0].value = format!("{}/{}", hp, max_hp);
                    }

                    // Update XP UI
                    let xp_pct = if max_xp > 0 { (xp as f32 / max_xp as f32).clamp(0.0, 1.0) * 100.0 } else { 0.0 };
                    if let Ok(mut style) = xp_fill_q.get_single_mut() {
                        style.width = Val::Percent(xp_pct);
                    }
                    if let Ok(mut text) = xp_text_q.get_single_mut() {
                        text.sections[0].value = format!("Niv {}  {}/{}", lvl, xp, max_xp);
                    }
                }
            }
        }
    }
}
