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

#[derive(Resource, Default)]
pub struct SelectedItem {
    pub id: Option<String>,
    pub name: Option<String>,
    pub is_equipped: bool,
}

fn setup_inventory_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
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
            // Main Inventory Window
            parent.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(850.0), // Agrandit pour faire de la place
                    height: Val::Px(550.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(20.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                background_color: Color::rgba(0.1, 0.1, 0.1, 0.95).into(),
                border_color: Color::rgba(0.8, 0.8, 0.8, 1.0).into(),
                ..default()
            }).with_children(|window| {
                // Title
                window.spawn(TextBundle::from_section(
                    "Inventaire",
                    TextStyle { font_size: 30.0, color: Color::WHITE, ..default() },
                ).with_style(Style {
                    margin: UiRect { bottom: Val::Px(20.0), ..default() },
                    ..default()
                }));
                
                // Split Panel
                window.spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        ..default()
                    },
                    ..default()
                }).with_children(|split| {
                    // Left Panel (Equipment Silhouette)
                    split.spawn((
                        NodeBundle {
                            style: Style {
                                width: Val::Px(150.0),
                                height: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceEvenly,
                                margin: UiRect { right: Val::Px(20.0), ..default() },
                                padding: UiRect::all(Val::Px(10.0)),
                                border: UiRect::right(Val::Px(2.0)),
                                ..default()
                            },
                            border_color: Color::rgba(0.5, 0.5, 0.5, 0.5).into(),
                            ..default()
                        },
                        EquipmentPanel,
                    )).with_children(|equip| {
                        let slots = vec!["head", "chest", "legs", "weapon"];
                        let labels = vec!["Casque", "Torse", "Jambes", "Arme"];
                        for (i, slot) in slots.iter().enumerate() {
                            equip.spawn((
                                ButtonBundle {
                                    style: Style {
                                        width: Val::Px(80.0),
                                        height: Val::Px(80.0),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        border: UiRect::all(Val::Px(1.0)),
                                        ..default()
                                    },
                                    background_color: Color::rgba(0.2, 0.2, 0.2, 1.0).into(),
                                    border_color: Color::rgba(0.4, 0.4, 0.4, 1.0).into(),
                                    ..default()
                                },
                                EquipmentSlot(slot.to_string()),
                            )).with_children(|btn| {
                                btn.spawn(TextBundle::from_section(
                                    labels[i],
                                    TextStyle { font_size: 14.0, color: Color::GRAY, ..default() },
                                ));
                            });
                        }
                    });

                    // Right Panel (Grid container)
                    split.spawn((
                        NodeBundle {
                            style: Style {
                                flex_grow: 1.0,
                                height: Val::Percent(100.0),
                                display: Display::Grid,
                                grid_template_columns: vec![GridTrack::flex(1.0); 5],
                                grid_template_rows: vec![GridTrack::flex(1.0); 4],
                                row_gap: Val::Px(10.0),
                                column_gap: Val::Px(10.0),
                                ..default()
                            },
                            ..default()
                        },
                        InventoryGrid,
                    ));
                });
                
                // Bottom Action & Description Bar
                window.spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        margin: UiRect { top: Val::Px(15.0), ..default() },
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    ..default()
                }).with_children(|bottom| {
                    bottom.spawn((
                        TextBundle::from_section(
                            "Survolez ou cliquez sur un objet pour voir ses détails.",
                            TextStyle { font_size: 16.0, color: Color::WHITE, ..default() },
                        ),
                        InventoryDescriptionText,
                    ));
                    
                    bottom.spawn((
                        ButtonBundle {
                            style: Style {
                                width: Val::Px(120.0),
                                height: Val::Px(35.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                display: Display::None,
                                ..default()
                            },
                            background_color: Color::rgba(0.2, 0.6, 0.2, 1.0).into(),
                            ..default()
                        },
                        InventoryActionBtn,
                    )).with_children(|btn| {
                        btn.spawn((
                            TextBundle::from_section(
                                "Equiper",
                                TextStyle { font_size: 16.0, color: Color::WHITE, ..default() },
                            ),
                            InventoryActionText,
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
    sender: Res<crate::net::NetworkSender>,
) {
    if console.open {
        return;
    }
    
    if keys.just_pressed(KeyCode::KeyI) {
        state.open = !state.open;
        if let Ok(mut visibility) = query.get_single_mut() {
            if state.open {
                *visibility = Visibility::Inherited;
                let _ = sender.0.send("INVENTORY\n".to_string());
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
                    
                    let mut icon = None;
                    if slot_type == "weapon" {
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
                                background_color: Color::rgba(0.2, 0.2, 0.2, 1.0).into(),
                                border_color: Color::rgba(0.4, 0.4, 0.4, 1.0).into(),
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
                                    TextStyle { font_size: 16.0, color: Color::WHITE, ..default() },
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
                *color = Color::rgba(0.4, 0.4, 0.4, 1.0).into(); // Highlight
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
                *color = Color::rgba(0.3, 0.3, 0.3, 1.0).into(); // Lighten on hover
            }
            Interaction::None => {
                *color = Color::rgba(0.2, 0.2, 0.2, 1.0).into(); // Normal color
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
                *color = Color::rgba(0.2, 0.8, 0.2, 1.0).into();
                if let Some(id) = &selected_item.id {
                    let cmd = if selected_item.is_equipped { "UNEQUIP" } else { "EQUIP" };
                    let _ = sender.0.send(format!("{} {}\n", cmd, id));
                }
            }
            Interaction::Hovered => {
                *color = Color::rgba(0.3, 0.7, 0.3, 1.0).into();
            }
            Interaction::None => {
                *color = Color::rgba(0.2, 0.6, 0.2, 1.0).into();
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
                    
                    if id != "none" && parts.len() >= 4 {
                        is_equipped = true;
                        item_name = parts[2].to_string();
                        damage = parts[3].parse().unwrap_or(0);
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
                                if slot_name == "weapon" {
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
                                            TextStyle { font_size: 14.0, color: Color::WHITE, ..default() },
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
                                        TextStyle { font_size: 14.0, color: Color::GRAY, ..default() },
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
