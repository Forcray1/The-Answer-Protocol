use bevy::prelude::*;

use crate::net::{NetworkSender, ServerMessageEvent};
use crate::AppState;

const MAX_CHAT_LINES: usize = 8;

pub struct ConsolePlugin;
pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryState>()
            .add_systems(Startup, setup_inventory_ui)
            .add_systems(Update, toggle_inventory.run_if(in_state(AppState::InGame)));
    }
}

#[derive(Resource, Default)]
pub struct InventoryState {
    pub open: bool,
}

#[derive(Component)]
struct InventoryUiRoot;

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
            parent.spawn(ImageBundle {
                style: Style {
                    width: Val::Percent(60.0), // Augmente la taille pour qu'elle prenne 60% de la largeur de l'écran
                    height: Val::Auto,         // Garde les proportions de l'image
                    ..default()
                },
                image: UiImage::new(asset_server.load("UI/inventory-ui.png")),
                ..default()
            });
        });
}

fn toggle_inventory(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<InventoryState>,
    mut query: Query<&mut Visibility, With<InventoryUiRoot>>,
    console: Res<ChatConsole>,
) {
    if console.open {
        return;
    }
    
    if keys.just_pressed(KeyCode::KeyI) {
        state.open = !state.open;
        if let Ok(mut visibility) = query.get_single_mut() {
            *visibility = if state.open { Visibility::Inherited } else { Visibility::Hidden };
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
