use bevy::prelude::*;

use crate::combat::CombatResource;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::LocalPlayerName;
use crate::AppState;

const MAX_CHAT_LINES: usize = 10;

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
    if !console.open && !console.terminal_open && !quest_state.open && keys.just_pressed(KeyCode::KeyI) {
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
            .init_resource::<ChatHistory>()
            .add_systems(Startup, setup_chat_ui)
            .add_systems(OnEnter(AppState::InGame), on_enter_ingame)
            .add_systems(OnExit(AppState::InGame), on_exit_ingame)
            .add_systems(
                Update,
                (
                    toggle_chat,
                    toggle_terminal,
                    handle_inputs.after(toggle_chat),
                    handle_terminal_inputs.after(toggle_terminal),
                    handle_tab_clicks,
                    handle_input_clicks,
                    process_chat_events,
                    display_terminal_messages,
                    update_chat_container_style,
                    update_chat_tabs_ui,
                    update_chat_input_ui,
                    update_chat_messages_ui,
                    spawn_chat_bubbles,
                    tick_chat_bubbles,
                    update_chat_combat_visibility,
                ).run_if(in_state(AppState::InGame)),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatChannel {
    #[default]
    Global,
    Room,
    Group,
}

impl ChatChannel {
    pub fn name(&self) -> &'static str {
        match self {
            ChatChannel::Global => "Global",
            ChatChannel::Room => "Room",
            ChatChannel::Group => "Group",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            ChatChannel::Global => Color::rgb(1.00, 0.85, 0.30),
            ChatChannel::Room => Color::rgb(0.35, 0.90, 0.55),
            ChatChannel::Group => Color::rgb(0.40, 0.72, 1.00),
        }
    }
}

#[derive(Resource)]
pub struct ChatConsole {
    pub open: bool,
    pub just_opened: bool,
    pub terminal_open: bool,
    pub terminal_just_opened: bool,
    pub active_channel: ChatChannel,
    pub in_group: bool,
    pub input_buffer: String,
    pub unread_global: u32,
    pub unread_room: u32,
    pub unread_group: u32,
    pub cursor_timer: Timer,
    pub cursor_visible: bool,
    pub last_sent: Option<(ChatChannel, String, std::time::Instant)>,
    pub room_change_time: Option<std::time::Instant>,
}

impl Default for ChatConsole {
    fn default() -> Self {
        Self {
            open: false,
            just_opened: false,
            terminal_open: false,
            terminal_just_opened: false,
            active_channel: ChatChannel::Global,
            in_group: false,
            input_buffer: String::new(),
            unread_global: 0,
            unread_room: 0,
            unread_group: 0,
            cursor_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
            cursor_visible: true,
            last_sent: None,
            room_change_time: None,
        }
    }
}

impl ChatConsole {
    pub fn cycle_channel(&mut self) {
        self.active_channel = match self.active_channel {
            ChatChannel::Global => ChatChannel::Room,
            ChatChannel::Room => {
                if self.in_group {
                    ChatChannel::Group
                } else {
                    ChatChannel::Global
                }
            }
            ChatChannel::Group => ChatChannel::Global,
        };
        self.clear_unread_for_active();
    }

    pub fn clear_unread_for_active(&mut self) {
        match self.active_channel {
            ChatChannel::Global => self.unread_global = 0,
            ChatChannel::Room => self.unread_room = 0,
            ChatChannel::Group => self.unread_group = 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub channel: ChatChannel,
    pub text: String,
    pub color: Color,
}

#[derive(Resource, Default)]
pub struct ChatHistory {
    pub messages: Vec<ChatMessage>,
}

impl ChatHistory {
    pub fn add(&mut self, channel: ChatChannel, text: String, color: Color) {
        self.messages.push(ChatMessage { channel, text, color });
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
    }
}

// ── Chatbox Components ──
#[derive(Component)]
pub struct ChatUiRoot;

#[derive(Component)]
pub struct ChatText;

#[derive(Component)]
pub struct ChatInputPrompt;

#[derive(Component)]
pub struct ChatInputText;

#[derive(Component)]
pub struct ChatInputContainer;

#[derive(Component)]
pub struct ChatTabButton(pub ChatChannel);

#[derive(Component)]
pub struct ChatTabText(pub ChatChannel);

// ── Developer Terminal Components ──
#[derive(Component)]
pub struct TerminalUiRoot;

#[derive(Component)]
pub struct TerminalLogText;

#[derive(Component)]
pub struct TerminalInputText;

fn setup_chat_ui(mut commands: Commands) {
    // ══════════════════════════════════════════════════════════════════════════
    // 1. CHATBOX JOUEUR (En bas à gauche)
    // ══════════════════════════════════════════════════════════════════════════
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(15.0),
                    bottom: Val::Px(15.0),
                    width: Val::Px(550.0),
                    height: Val::Px(245.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::all(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                background_color: Color::rgba(0.05, 0.06, 0.09, 0.70).into(),
                border_color: Color::rgba(0.25, 0.25, 0.35, 0.45).into(),
                visibility: Visibility::Hidden,
                z_index: ZIndex::Global(10),
                ..default()
            },
            ChatUiRoot,
        ))
        .with_children(|root| {
            // ── Tabs row ──
            root.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Px(26.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    margin: UiRect { bottom: Val::Px(4.0), ..default() },
                    ..default()
                },
                ..default()
            })
            .with_children(|tabs_row| {
                tabs_row
                    .spawn(NodeBundle {
                        style: Style {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(6.0),
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        ..default()
                    })
                    .with_children(|tabs| {
                        // Global tab button
                        tabs.spawn((
                            ButtonBundle {
                                style: Style {
                                    padding: UiRect { left: Val::Px(10.0), right: Val::Px(10.0), top: Val::Px(3.0), bottom: Val::Px(3.0) },
                                    border: UiRect::all(Val::Px(1.0)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                background_color: Color::rgba(0.20, 0.22, 0.30, 0.95).into(),
                                border_color: ChatChannel::Global.color().into(),
                                ..default()
                            },
                            ChatTabButton(ChatChannel::Global),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                TextBundle::from_section(
                                    "Global",
                                    TextStyle { font_size: 13.0, color: ChatChannel::Global.color(), ..default() },
                                ),
                                ChatTabText(ChatChannel::Global),
                            ));
                        });

                        // Room tab button
                        tabs.spawn((
                            ButtonBundle {
                                style: Style {
                                    padding: UiRect { left: Val::Px(10.0), right: Val::Px(10.0), top: Val::Px(3.0), bottom: Val::Px(3.0) },
                                    border: UiRect::all(Val::Px(1.0)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                background_color: Color::rgba(0.08, 0.08, 0.12, 0.60).into(),
                                border_color: Color::rgba(0.20, 0.20, 0.28, 0.40).into(),
                                ..default()
                            },
                            ChatTabButton(ChatChannel::Room),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                TextBundle::from_section(
                                    "Room",
                                    TextStyle { font_size: 13.0, color: Color::rgba(0.65, 0.65, 0.70, 0.8), ..default() },
                                ),
                                ChatTabText(ChatChannel::Room),
                            ));
                        });

                        // Group tab button (hidden until player is in a group)
                        tabs.spawn((
                            ButtonBundle {
                                style: Style {
                                    display: Display::None,
                                    padding: UiRect { left: Val::Px(10.0), right: Val::Px(10.0), top: Val::Px(3.0), bottom: Val::Px(3.0) },
                                    border: UiRect::all(Val::Px(1.0)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                background_color: Color::rgba(0.08, 0.08, 0.12, 0.60).into(),
                                border_color: Color::rgba(0.20, 0.20, 0.28, 0.40).into(),
                                ..default()
                            },
                            ChatTabButton(ChatChannel::Group),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                TextBundle::from_section(
                                    "Group",
                                    TextStyle { font_size: 13.0, color: Color::rgba(0.65, 0.65, 0.70, 0.8), ..default() },
                                ),
                                ChatTabText(ChatChannel::Group),
                            ));
                        });
                    });

                tabs_row.spawn(TextBundle::from_section(
                    "[Tab] Canal  [T] Parler  [F1] Console Dev",
                    TextStyle { font_size: 11.0, color: Color::rgba(0.55, 0.55, 0.62, 0.7), ..default() },
                ));
            });

            // ── Messages Area ──
            root.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    margin: UiRect { top: Val::Px(2.0), bottom: Val::Px(4.0), ..default() },
                    padding: UiRect::all(Val::Px(6.0)),
                    flex_direction: FlexDirection::ColumnReverse,
                    overflow: Overflow::clip(),
                    ..default()
                },
                background_color: Color::rgba(0.02, 0.03, 0.05, 0.50).into(),
                ..default()
            })
            .with_children(|msg_area| {
                msg_area.spawn((
                    TextBundle::from_section(
                        "Bienvenue ! Écrivez directement pour discuter.\n",
                        TextStyle { font_size: 14.0, color: Color::rgba(0.85, 0.85, 0.90, 1.0), ..default() },
                    ),
                    ChatText,
                ));
            });

            // ── Input Area ──
            root.spawn((
                ButtonBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Px(32.0),
                        padding: UiRect { left: Val::Px(8.0), right: Val::Px(8.0), top: Val::Px(3.0), bottom: Val::Px(3.0) },
                        border: UiRect::all(Val::Px(1.0)),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    background_color: Color::rgba(0.04, 0.05, 0.08, 0.80).into(),
                    border_color: Color::rgba(0.25, 0.25, 0.35, 0.50).into(),
                    ..default()
                },
                ChatInputContainer,
            ))
            .with_children(|input_box| {
                input_box.spawn((
                    TextBundle::from_section(
                        "[Global] ",
                        TextStyle { font_size: 14.0, color: ChatChannel::Global.color(), ..default() },
                    ),
                    ChatInputPrompt,
                ));
                input_box.spawn((
                    TextBundle::from_section(
                        "Appuyez sur 'T' pour parler...",
                        TextStyle { font_size: 13.0, color: Color::rgba(0.55, 0.55, 0.60, 0.7), ..default() },
                    ),
                    ChatInputText,
                ));
            });
        });

    // ══════════════════════════════════════════════════════════════════════════
    // 2. TERMINAL DÉVELOPPEUR (Touche F1 ou Backquote `~`/`²`)
    // ══════════════════════════════════════════════════════════════════════════
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    top: Val::Px(20.0),
                    left: Val::Px(20.0),
                    width: Val::Px(640.0),
                    height: Val::Px(290.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                background_color: Color::rgba(0.02, 0.02, 0.04, 0.94).into(),
                border_color: Color::rgba(0.85, 0.70, 0.20, 0.80).into(),
                visibility: Visibility::Hidden,
                z_index: ZIndex::Global(100),
                ..default()
            },
            TerminalUiRoot,
        ))
        .with_children(|term| {
            // Header
            term.spawn(TextBundle::from_section(
                "[TERMINAL DÉVELOPPEUR] Commandes: LOOK, MOVE, ATTACK, WHO, STATUS, etc.  Fermer: [F1] / [Échap]",
                TextStyle { font_size: 13.0, color: Color::YELLOW, ..default() },
            ));

            // Log Area
            term.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    margin: UiRect { top: Val::Px(6.0), bottom: Val::Px(6.0), ..default() },
                    padding: UiRect::all(Val::Px(8.0)),
                    flex_direction: FlexDirection::ColumnReverse,
                    overflow: Overflow::clip(),
                    ..default()
                },
                background_color: Color::rgba(0.0, 0.0, 0.0, 0.85).into(),
                ..default()
            })
            .with_children(|log_area| {
                log_area.spawn((
                    TextBundle::from_section(
                        "Connecté au serveur.\nTapez vos commandes ci-dessous.\n",
                        TextStyle { font_size: 14.0, color: Color::WHITE, ..default() },
                    ),
                    TerminalLogText,
                ));
            });

            // Input Bar
            term.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Px(34.0),
                    padding: UiRect { left: Val::Px(10.0), right: Val::Px(10.0), top: Val::Px(5.0), bottom: Val::Px(5.0) },
                    border: UiRect::all(Val::Px(1.0)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                background_color: Color::rgba(0.1, 0.1, 0.12, 0.95).into(),
                border_color: Color::rgba(0.4, 0.4, 0.5, 0.6).into(),
                ..default()
            })
            .with_children(|input_bar| {
                input_bar.spawn((
                    TextBundle::from_section(
                        "> ",
                        TextStyle { font_size: 16.0, color: Color::YELLOW, ..default() },
                    ),
                    TerminalInputText,
                ));
            });
        });
}

fn on_enter_ingame(
    mut chat_root_q: Query<&mut Visibility, (With<ChatUiRoot>, Without<TerminalUiRoot>)>,
    sender: Res<NetworkSender>,
    combat: Option<Res<CombatResource>>,
) {
    let in_combat = combat.as_ref().map_or(false, |c| c.in_combat);
    if let Ok(mut vis) = chat_root_q.get_single_mut() {
        *vis = if in_combat { Visibility::Hidden } else { Visibility::Inherited };
    }
    let _ = sender.0.send("GROUP\n".to_string());
}

fn on_exit_ingame(
    mut chat_root_q: Query<&mut Visibility, (With<ChatUiRoot>, Without<TerminalUiRoot>)>,
    mut term_root_q: Query<&mut Visibility, (With<TerminalUiRoot>, Without<ChatUiRoot>)>,
    mut console: ResMut<ChatConsole>,
) {
    if let Ok(mut vis) = chat_root_q.get_single_mut() {
        *vis = Visibility::Hidden;
    }
    if let Ok(mut vis) = term_root_q.get_single_mut() {
        *vis = Visibility::Hidden;
    }
    console.open = false;
    console.terminal_open = false;
    console.input_buffer.clear();
}

fn toggle_chat(
    keys: Res<ButtonInput<KeyCode>>,
    mut console: ResMut<ChatConsole>,
    combat: Option<Res<CombatResource>>,
) {
    if let Some(ref combat) = combat {
        if combat.in_combat {
            return;
        }
    }
    if console.terminal_open {
        return;
    }
    if !console.open {
        if keys.just_pressed(KeyCode::KeyT) || keys.just_pressed(KeyCode::Enter) {
            console.open = true;
            console.just_opened = true;
        } else if keys.just_pressed(KeyCode::Slash) {
            console.open = true;
            console.just_opened = true;
            console.input_buffer = "/".to_string();
        }
    } else {
        if keys.just_pressed(KeyCode::Escape) {
            console.open = false;
            console.input_buffer.clear();
        } else if keys.just_pressed(KeyCode::Tab) {
            console.cycle_channel();
        }
    }
}

fn toggle_terminal(
    keys: Res<ButtonInput<KeyCode>>,
    mut console: ResMut<ChatConsole>,
    mut query: Query<&mut Visibility, With<TerminalUiRoot>>,
    combat: Option<Res<CombatResource>>,
) {
    if let Some(ref combat) = combat {
        if combat.in_combat {
            return;
        }
    }
    let toggle_pressed = keys.just_pressed(KeyCode::F1)
        || keys.just_pressed(KeyCode::Backquote)
        || keys.just_pressed(KeyCode::F2);

    if toggle_pressed {
        console.terminal_open = !console.terminal_open;
        console.terminal_just_opened = console.terminal_open;
        if console.terminal_open {
            console.open = false;
        }
        if let Ok(mut vis) = query.get_single_mut() {
            *vis = if console.terminal_open { Visibility::Inherited } else { Visibility::Hidden };
        }
    } else if console.terminal_open && keys.just_pressed(KeyCode::Escape) {
        console.terminal_open = false;
        if let Ok(mut vis) = query.get_single_mut() {
            *vis = Visibility::Hidden;
        }
    }
}

fn handle_tab_clicks(
    interaction_q: Query<(&Interaction, &ChatTabButton), (Changed<Interaction>, With<Button>)>,
    mut console: ResMut<ChatConsole>,
    combat: Option<Res<CombatResource>>,
) {
    if let Some(ref combat) = combat {
        if combat.in_combat {
            return;
        }
    }
    for (interaction, btn) in &interaction_q {
        if *interaction == Interaction::Pressed {
            console.active_channel = btn.0;
            console.clear_unread_for_active();
        }
    }
}

fn handle_input_clicks(
    interaction_q: Query<&Interaction, (Changed<Interaction>, With<ChatInputContainer>)>,
    mut console: ResMut<ChatConsole>,
    combat: Option<Res<CombatResource>>,
) {
    if let Some(ref combat) = combat {
        if combat.in_combat {
            return;
        }
    }
    for interaction in &interaction_q {
        if *interaction == Interaction::Pressed {
            console.open = true;
        }
    }
}

fn handle_terminal_inputs(
    mut char_evr: EventReader<ReceivedCharacter>,
    keys: Res<ButtonInput<KeyCode>>,
    sender: Res<NetworkSender>,
    mut console: ResMut<ChatConsole>,
    mut input_query: Query<&mut Text, (With<TerminalInputText>, Without<TerminalLogText>)>,
    mut log_query: Query<&mut Text, (With<TerminalLogText>, Without<TerminalInputText>)>,
    combat: Option<Res<CombatResource>>,
) {
    if let Some(ref combat) = combat {
        if combat.in_combat {
            return;
        }
    }
    if !console.terminal_open {
        return;
    }
    let swallow_chars = std::mem::take(&mut console.terminal_just_opened);

    let Ok(mut input_text) = input_query.get_single_mut() else {
        return;
    };

    for ev in char_evr.read() {
        if swallow_chars {
            continue;
        }
        let s = ev.char.to_string();
        if !s.contains('\u{8}') && !s.contains('\r') && !s.contains('\n') && !s.contains('`') && !s.contains('~') && !s.contains('²') {
            input_text.sections[0].value.push_str(&s);
        }
    }

    if keys.just_pressed(KeyCode::Backspace) && input_text.sections[0].value.chars().count() > 2 {
        input_text.sections[0].value.pop();
    }

    if keys.just_pressed(KeyCode::Enter) {
        let command = input_text.sections[0].value[2..].trim().to_string();
        if !command.is_empty() {
            let _ = sender.0.send(format!("{}\n", command));
            if let Ok(mut log_text) = log_query.get_single_mut() {
                log_text.sections[0].value.push_str(&format!("> {}\n", command));
                let lines: Vec<&str> = log_text.sections[0].value.lines().collect();
                if lines.len() > 15 {
                    log_text.sections[0].value = lines[lines.len() - 15..].join("\n") + "\n";
                }
            }
            input_text.sections[0].value = "> ".to_string();
        }
    }
}

fn display_terminal_messages(
    mut events: EventReader<ServerMessageEvent>,
    mut query: Query<&mut Text, With<TerminalLogText>>,
) {
    for ev in events.read() {
        for mut text in query.iter_mut() {
            text.sections[0].value.push_str(&format!("{}\n", ev.0.trim()));
            let lines: Vec<&str> = text.sections[0].value.lines().collect();
            if lines.len() > 15 {
                text.sections[0].value = lines[lines.len() - 15..].join("\n") + "\n";
            }
        }
    }
}

fn handle_inputs(
    mut char_evr: EventReader<ReceivedCharacter>,
    keys: Res<ButtonInput<KeyCode>>,
    sender: Res<NetworkSender>,
    mut console: ResMut<ChatConsole>,
    mut history: ResMut<ChatHistory>,
    local_name: Res<LocalPlayerName>,
    combat: Option<Res<CombatResource>>,
) {
    if let Some(ref combat) = combat {
        if combat.in_combat {
            return;
        }
    }
    if !console.open || console.terminal_open {
        return;
    }
    let swallow_chars = std::mem::take(&mut console.just_opened);

    for ev in char_evr.read() {
        if swallow_chars {
            continue;
        }
        let s = ev.char.to_string();
        if !s.contains('\u{8}') && !s.contains('\r') && !s.contains('\n') && !s.contains('\t') {
            console.input_buffer.push_str(&s);
        }
    }

    if keys.just_pressed(KeyCode::Backspace) {
        console.input_buffer.pop();
    }

    if keys.just_pressed(KeyCode::Enter) {
        let msg = console.input_buffer.trim().to_string();
        if !msg.is_empty() {
            let my_name = local_name.0.clone().unwrap_or_else(|| "Moi".to_string());
            if msg.starts_with('/') {
                let cmd = msg[1..].trim();
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                let first = parts.first().map(|s| s.to_uppercase()).unwrap_or_default();

                match first.as_str() {
                    "HELP" => {
                        history.add(
                            console.active_channel,
                            "Commandes: /group (create|invite <nom>|accept|leave), /who, /global <msg>, /room <msg>, /group <msg>".to_string(),
                            Color::rgb(0.95, 0.85, 0.40),
                        );
                    }
                    "INVITE" if parts.len() > 1 => {
                        let _ = sender.0.send(format!("GROUP INVITE {}\n", parts[1..].join(" ")));
                    }
                    "ACCEPT" => {
                        let _ = sender.0.send("GROUP ACCEPT\n".to_string());
                    }
                    "LEAVE" => {
                        let _ = sender.0.send("GROUP LEAVE\n".to_string());
                    }
                    "GLOBAL" if parts.len() > 1 => {
                        let text = parts[1..].join(" ");
                        let _ = sender.0.send(format!("CHAT GLOBAL {}\n", text));
                        history.add(ChatChannel::Global, format!("[Global] {}: {}", my_name, text), Color::rgb(0.95, 0.90, 0.80));
                        console.last_sent = Some((ChatChannel::Global, text, std::time::Instant::now()));
                    }
                    "ROOM" if parts.len() > 1 => {
                        let text = parts[1..].join(" ");
                        let _ = sender.0.send(format!("CHAT ROOM {}\n", text));
                        history.add(ChatChannel::Room, format!("[Room] {}: {}", my_name, text), Color::rgb(0.85, 1.0, 0.88));
                        console.last_sent = Some((ChatChannel::Room, text, std::time::Instant::now()));
                    }
                    "GROUP" if parts.len() > 1 && !["CREATE", "INVITE", "ACCEPT", "LEAVE", "INFO"].contains(&parts[1].to_uppercase().as_str()) => {
                        let text = parts[1..].join(" ");
                        let _ = sender.0.send(format!("CHAT GROUP {}\n", text));
                        history.add(ChatChannel::Group, format!("[Group] {}: {}", my_name, text), Color::rgb(0.80, 0.90, 1.0));
                        console.last_sent = Some((ChatChannel::Group, text, std::time::Instant::now()));
                    }
                    _ => {
                        let _ = sender.0.send(format!("{}\n", cmd));
                    }
                }
            } else if msg.to_uppercase().starts_with("CHAT ") {
                let _ = sender.0.send(format!("{}\n", msg));
            } else {
                let channel_cmd = match console.active_channel {
                    ChatChannel::Global => "GLOBAL",
                    ChatChannel::Room => "ROOM",
                    ChatChannel::Group => "GROUP",
                };
                let _ = sender.0.send(format!("CHAT {} {}\n", channel_cmd, msg));

                // Ajout immédiat pour un affichage instantané et garanti
                let (prefix, color) = match console.active_channel {
                    ChatChannel::Global => (format!("[Global] {}: {}", my_name, msg), Color::rgb(0.95, 0.90, 0.80)),
                    ChatChannel::Room => (format!("[Room] {}: {}", my_name, msg), Color::rgb(0.85, 1.0, 0.88)),
                    ChatChannel::Group => (format!("[Group] {}: {}", my_name, msg), Color::rgb(0.80, 0.90, 1.0)),
                };
                history.add(console.active_channel, prefix, color);
                console.last_sent = Some((console.active_channel, msg.clone(), std::time::Instant::now()));
            }
            console.input_buffer.clear();
        }
        console.open = false;
    }
}

fn process_chat_events(
    mut events: EventReader<ServerMessageEvent>,
    mut console: ResMut<ChatConsole>,
    mut history: ResMut<ChatHistory>,
    local_name: Res<LocalPlayerName>,
) {
    for ev in events.read() {
        let line = ev.0.trim();

        // 1. GLOBAL CHAT: S: EVT GLOBAL CHAT <sender> <message>
        if let Some(rest) = line.strip_prefix("S: EVT GLOBAL CHAT ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 2 {
                let sender = parts[0];
                let message = parts[1..].join(" ");
                let is_recent_self = local_name.0.as_deref() == Some(sender)
                    && console.last_sent.as_ref().map_or(false, |(c, m, t)| *c == ChatChannel::Global && m == &message && t.elapsed().as_secs() < 3);

                if !is_recent_self {
                    if sender == "Server" {
                        history.add(
                            ChatChannel::Global,
                            format!("[Server] {}", message),
                            Color::rgb(1.0, 0.85, 0.3),
                        );
                    } else {
                        history.add(
                            ChatChannel::Global,
                            format!("[Global] {}: {}", sender, message),
                            Color::rgb(0.95, 0.90, 0.80),
                        );
                    }
                    if console.active_channel != ChatChannel::Global {
                        console.unread_global += 1;
                    }
                }
            }
            continue;
        }

        // 2. ROOM EVENTS: S: EVT ROOM <room> ...
        if line.starts_with("S: EVT ROOM ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                match parts[4] {
                    "CHAT" if parts.len() >= 7 => {
                        let sender = parts[5].replace('_', " ");
                        let message = parts[6..].join(" ");
                        let is_recent_self = local_name.0.as_deref() == Some(sender.as_str())
                            && console.last_sent.as_ref().map_or(false, |(c, m, t)| *c == ChatChannel::Room && m == &message && t.elapsed().as_secs() < 3);

                        if !is_recent_self {
                            history.add(
                                ChatChannel::Room,
                                format!("[Room] {}: {}", sender, message),
                                Color::rgb(0.85, 1.0, 0.88),
                            );
                            if console.active_channel != ChatChannel::Room {
                                console.unread_room += 1;
                            }
                        }
                    }
                    "PRESENCE" if parts.len() >= 7 => {
                        let action = parts[5];
                        let user = parts[6];
                        let is_me = local_name.0.as_deref() == Some(user);
                        if !is_me {
                            if action == "LEAVE" {
                                history.add(
                                    ChatChannel::Room,
                                    format!("[Room] {} left the room", user),
                                    Color::rgb(0.70, 0.70, 0.75),
                                );
                                if console.active_channel != ChatChannel::Room {
                                    console.unread_room += 1;
                                }
                            } else if action == "ENTER" {
                                let is_room_loading = console.room_change_time.as_ref().map_or(false, |t| t.elapsed().as_millis() < 800);
                                if !is_room_loading {
                                    history.add(
                                        ChatChannel::Room,
                                        format!("[Room] {} entered the room", user),
                                        Color::rgb(0.70, 0.70, 0.75),
                                    );
                                    if console.active_channel != ChatChannel::Room {
                                        console.unread_room += 1;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            continue;
        }

        // 3. ROOM LOCATION CHANGE: S: OK room-loc.<room>
        if let Some(room) = line.strip_prefix("S: OK room-loc.") {
            console.room_change_time = Some(std::time::Instant::now());
            history.add(
                ChatChannel::Room,
                format!("[Room] Arrivée dans : {}", room),
                Color::rgb(0.40, 0.80, 0.90),
            );
            continue;
        }

        if line.starts_with("S: OK connected") {
            console.room_change_time = Some(std::time::Instant::now());
        }

        // 4. GROUP EVENTS: S: EVT GROUP ...
        if line.starts_with("S: EVT GROUP ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                match parts[3] {
                    "CHAT" if parts.len() >= 6 => {
                        let sender = parts[4];
                        let message = parts[5..].join(" ");
                        let is_recent_self = local_name.0.as_deref() == Some(sender)
                            && console.last_sent.as_ref().map_or(false, |(c, m, t)| *c == ChatChannel::Group && m == &message && t.elapsed().as_secs() < 3);

                        if !is_recent_self {
                            history.add(
                                ChatChannel::Group,
                                format!("[Group] {}: {}", sender, message),
                                Color::rgb(0.80, 0.90, 1.0),
                            );
                            if console.active_channel != ChatChannel::Group {
                                console.unread_group += 1;
                            }
                        }
                    }
                    "JOIN" if parts.len() >= 5 => {
                        let user = parts[4];
                        history.add(
                            ChatChannel::Group,
                            format!("[Group] {} a rejoint le groupe", user),
                            Color::rgb(0.50, 0.85, 1.0),
                        );
                        if local_name.0.as_deref() == Some(user) {
                            console.in_group = true;
                        }
                    }
                    "LEAVE" if parts.len() >= 5 => {
                        let user = parts[4];
                        history.add(
                            ChatChannel::Group,
                            format!("[Group] {} a quitté le groupe", user),
                            Color::rgb(0.70, 0.75, 0.90),
                        );
                        if local_name.0.as_deref() == Some(user) {
                            console.in_group = false;
                            if console.active_channel == ChatChannel::Group {
                                console.active_channel = ChatChannel::Room;
                            }
                        }
                    }
                    "CREATED" if parts.len() >= 5 => {
                        let user = parts[4];
                        history.add(
                            ChatChannel::Group,
                            format!("[Group] {} a créé le groupe", user),
                            Color::rgb(0.50, 0.85, 1.0),
                        );
                        if local_name.0.as_deref() == Some(user) {
                            console.in_group = true;
                        }
                    }
                    "DISBAND" => {
                        let text = parts[4..].join(" ");
                        history.add(
                            ChatChannel::Group,
                            format!("[Group] {}", text),
                            Color::rgb(1.0, 0.6, 0.4),
                        );
                        history.add(
                            ChatChannel::Room,
                            format!("[Group] {}", text),
                            Color::rgb(1.0, 0.6, 0.4),
                        );
                        console.in_group = false;
                        if console.active_channel == ChatChannel::Group {
                            console.active_channel = ChatChannel::Room;
                        }
                    }
                    "INVITED" => {
                        let text = parts[4..].join(" ");
                        let msg = format!("[Group] {}", text);
                        history.add(ChatChannel::Room, msg.clone(), Color::rgb(1.0, 0.85, 0.4));
                        history.add(ChatChannel::Global, msg, Color::rgb(1.0, 0.85, 0.4));
                    }
                    _ => {}
                }
            }
            continue;
        }

        // 5. SERVER RESPONSES
        if line == "S: OK you_joined_the_group" {
            console.in_group = true;
            history.add(ChatChannel::Room, "[Group] Vous avez rejoint le groupe !".to_string(), Color::rgb(0.5, 0.85, 1.0));
            history.add(ChatChannel::Group, "[Group] Vous avez rejoint le groupe !".to_string(), Color::rgb(0.5, 0.85, 1.0));
        } else if line == "S: OK group_created" {
            console.in_group = true;
            history.add(ChatChannel::Room, "[Group] Groupe créé !".to_string(), Color::rgb(0.5, 0.85, 1.0));
            history.add(ChatChannel::Group, "[Group] Groupe créé !".to_string(), Color::rgb(0.5, 0.85, 1.0));
        } else if line == "S: OK you_left_the_group" {
            console.in_group = false;
            if console.active_channel == ChatChannel::Group {
                console.active_channel = ChatChannel::Room;
            }
            history.add(ChatChannel::Room, "[Group] Vous avez quitté le groupe.".to_string(), Color::rgb(0.7, 0.7, 0.8));
        } else if line == "S: OK group_disbanded" {
            console.in_group = false;
            if console.active_channel == ChatChannel::Group {
                console.active_channel = ChatChannel::Room;
            }
            history.add(ChatChannel::Room, "[Group] Le groupe a été dissous.".to_string(), Color::rgb(1.0, 0.6, 0.4));
        } else if let Some(members) = line.strip_prefix("S: OK Group members: ") {
            console.in_group = true;
            history.add(ChatChannel::Group, format!("[Group] Membres : {}", members), Color::rgb(0.5, 0.85, 1.0));
        } else if line == "S: OK No group" {
            console.in_group = false;
            if console.active_channel == ChatChannel::Group {
                console.active_channel = ChatChannel::Room;
            }
        } else if let Some(target) = line.strip_prefix("S: OK you invited ") {
            history.add(console.active_channel, format!("[Group] Invitation envoyée à {}", target), Color::rgb(1.0, 0.85, 0.4));
        } else if line == "S: ERR chat_spam_forbidden" {
            history.add(console.active_channel, "[System] Ralentissez ! Anti-spam actif (2s).".to_string(), Color::rgb(1.0, 0.4, 0.4));
        } else if line == "S: ERR you_have_no_group" {
            console.in_group = false;
            if console.active_channel == ChatChannel::Group {
                console.active_channel = ChatChannel::Room;
            }
            history.add(console.active_channel, "[System] Vous n'êtes pas dans un groupe.".to_string(), Color::rgb(1.0, 0.4, 0.4));
        } else if line == "S: ERR player_not_found" {
            history.add(console.active_channel, "[System] Joueur introuvable.".to_string(), Color::rgb(1.0, 0.4, 0.4));
        } else if line == "S: ERR player_already_in_group" {
            history.add(console.active_channel, "[System] Ce joueur est déjà dans un groupe.".to_string(), Color::rgb(1.0, 0.4, 0.4));
        } else if line == "S: ERR cannot_invite_yourself" {
            history.add(console.active_channel, "[System] Impossible de vous inviter vous-même.".to_string(), Color::rgb(1.0, 0.4, 0.4));
        } else if line == "S: ERR you_already_have_a_group" {
            history.add(console.active_channel, "[System] Vous avez déjà un groupe.".to_string(), Color::rgb(1.0, 0.4, 0.4));
        } else if line == "S: ERR no_pending_invite" {
            history.add(console.active_channel, "[System] Aucune invitation en attente.".to_string(), Color::rgb(1.0, 0.4, 0.4));
        } else if line == "S: ERR group_no_longer_exists" {
            history.add(console.active_channel, "[System] Le groupe n'existe plus.".to_string(), Color::rgb(1.0, 0.4, 0.4));
        }
    }
}

fn update_chat_combat_visibility(
    combat: Option<Res<CombatResource>>,
    mut chat_root_q: Query<&mut Visibility, (With<ChatUiRoot>, Without<TerminalUiRoot>)>,
    mut term_root_q: Query<&mut Visibility, (With<TerminalUiRoot>, Without<ChatUiRoot>)>,
    mut console: ResMut<ChatConsole>,
) {
    let Some(combat) = combat else { return };
    if combat.is_changed() {
        if combat.in_combat {
            if let Ok(mut vis) = chat_root_q.get_single_mut() {
                *vis = Visibility::Hidden;
            }
            if let Ok(mut vis) = term_root_q.get_single_mut() {
                *vis = Visibility::Hidden;
            }
            console.open = false;
            console.terminal_open = false;
            console.input_buffer.clear();
        } else {
            if let Ok(mut vis) = chat_root_q.get_single_mut() {
                *vis = Visibility::Inherited;
            }
        }
    }
}

fn update_chat_container_style(
    console: Res<ChatConsole>,
    mut query: Query<(&mut BackgroundColor, &mut BorderColor), With<ChatUiRoot>>,
) {
    if !console.is_changed() {
        return;
    }
    if let Ok((mut bg, mut border)) = query.get_single_mut() {
        if console.open {
            *bg = Color::rgba(0.06, 0.07, 0.10, 0.92).into();
            *border = Color::rgba(0.45, 0.45, 0.55, 0.85).into();
        } else {
            *bg = Color::rgba(0.04, 0.05, 0.07, 0.65).into();
            *border = Color::rgba(0.25, 0.25, 0.35, 0.45).into();
        }
    }
}

fn update_chat_tabs_ui(
    console: Res<ChatConsole>,
    mut tab_buttons: Query<(&ChatTabButton, &mut BackgroundColor, &mut BorderColor, &mut Style)>,
    mut tab_texts: Query<(&ChatTabText, &mut Text)>,
) {
    for (btn, mut bg, mut border, mut style) in &mut tab_buttons {
        if btn.0 == ChatChannel::Group {
            style.display = if console.in_group { Display::Flex } else { Display::None };
        }

        if btn.0 == console.active_channel {
            *bg = Color::rgba(0.20, 0.22, 0.30, 0.95).into();
            *border = btn.0.color().into();
        } else {
            *bg = Color::rgba(0.08, 0.08, 0.12, 0.60).into();
            *border = Color::rgba(0.20, 0.20, 0.28, 0.40).into();
        }
    }

    for (tab_text, mut text) in &mut tab_texts {
        let is_active = tab_text.0 == console.active_channel;
        let unread = match tab_text.0 {
            ChatChannel::Global => console.unread_global,
            ChatChannel::Room => console.unread_room,
            ChatChannel::Group => console.unread_group,
        };

        if is_active {
            text.sections[0].value = tab_text.0.name().to_string();
            text.sections[0].style.color = tab_text.0.color();
        } else if unread > 0 {
            text.sections[0].value = format!("{} (*)", tab_text.0.name());
            text.sections[0].style.color = tab_text.0.color();
        } else {
            text.sections[0].value = tab_text.0.name().to_string();
            text.sections[0].style.color = Color::rgba(0.60, 0.60, 0.65, 0.75);
        }
    }
}

fn update_chat_input_ui(
    time: Res<Time>,
    mut console: ResMut<ChatConsole>,
    mut prompt_q: Query<&mut Text, (With<ChatInputPrompt>, Without<ChatInputText>)>,
    mut input_text_q: Query<&mut Text, (With<ChatInputText>, Without<ChatInputPrompt>)>,
    mut container_q: Query<(&mut BackgroundColor, &mut BorderColor), With<ChatInputContainer>>,
) {
    console.cursor_timer.tick(time.delta());
    if console.cursor_timer.just_finished() {
        console.cursor_visible = !console.cursor_visible;
    }

    let channel_color = console.active_channel.color();

    if let Ok(mut prompt) = prompt_q.get_single_mut() {
        prompt.sections[0].value = if console.open {
            format!("[{}] > ", console.active_channel.name())
        } else {
            format!("[{}] ", console.active_channel.name())
        };
        prompt.sections[0].style.color = if console.open {
            channel_color
        } else {
            channel_color.with_a(0.65)
        };
    }

    if let Ok(mut text) = input_text_q.get_single_mut() {
        if console.open {
            let cursor = if console.cursor_visible { "_" } else { " " };
            text.sections[0].value = format!("{}{}", console.input_buffer, cursor);
            text.sections[0].style.color = Color::WHITE;
            text.sections[0].style.font_size = 14.0;
        } else {
            text.sections[0].value = "Appuyez sur 'T' pour parler...".to_string();
            text.sections[0].style.color = Color::rgba(0.55, 0.55, 0.60, 0.7);
            text.sections[0].style.font_size = 13.0;
        }
    }

    if let Ok((mut bg, mut border)) = container_q.get_single_mut() {
        if console.open {
            *bg = Color::rgba(0.08, 0.09, 0.13, 0.95).into();
            *border = channel_color.into();
        } else {
            *bg = Color::rgba(0.04, 0.05, 0.08, 0.70).into();
            *border = Color::rgba(0.25, 0.25, 0.35, 0.50).into();
        }
    }
}

fn update_chat_messages_ui(
    console: Res<ChatConsole>,
    history: Res<ChatHistory>,
    mut query: Query<&mut Text, With<ChatText>>,
) {
    if !history.is_changed() && !console.is_changed() {
        return;
    }

    let Ok(mut text) = query.get_single_mut() else {
        return;
    };

    let relevant: Vec<&ChatMessage> = history
        .messages
        .iter()
        .filter(|m| m.channel == console.active_channel)
        .rev()
        .take(MAX_CHAT_LINES)
        .collect();

    let mut sections = Vec::new();
    if relevant.is_empty() {
        sections.push(TextSection::new(
            format!("(Aucun message dans le canal {})\n", console.active_channel.name()),
            TextStyle {
                font_size: 14.0,
                color: Color::rgba(0.5, 0.5, 0.55, 0.7),
                ..default()
            },
        ));
    } else {
        for msg in relevant.into_iter().rev() {
            sections.push(TextSection::new(
                format!("{}\n", msg.text),
                TextStyle {
                    font_size: 14.0,
                    color: msg.color,
                    ..default()
                },
            ));
        }
    }
    text.sections = sections;
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
    if !console.open && !console.terminal_open && !inventory.open && keys.just_pressed(KeyCode::KeyU) {
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
            .add_systems(
                Update,
                (
                    handle_player_stats,
                    update_hud_combat_visibility,
                ).run_if(in_state(AppState::InGame)),
            )
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

fn show_hud(
    mut q: Query<&mut Visibility, With<HudUiRoot>>,
    combat: Option<Res<CombatResource>>,
) {
    let in_combat = combat.as_ref().map_or(false, |c| c.in_combat);
    for mut vis in q.iter_mut() {
        *vis = if in_combat { Visibility::Hidden } else { Visibility::Inherited };
    }
}

fn hide_hud(mut q: Query<&mut Visibility, With<HudUiRoot>>) {
    for mut vis in q.iter_mut() {
        *vis = Visibility::Hidden;
    }
}

fn update_hud_combat_visibility(
    combat: Option<Res<CombatResource>>,
    mut q: Query<&mut Visibility, With<HudUiRoot>>,
) {
    let Some(combat) = combat else { return };
    if combat.is_changed() {
        for mut vis in q.iter_mut() {
            *vis = if combat.in_combat {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
        }
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
