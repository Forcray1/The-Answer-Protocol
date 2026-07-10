use bevy::prelude::*;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::thread;
use tokio::net::TcpStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

mod map;
mod player;


#[derive(Event)]
pub struct ServerMessageEvent(pub String);

#[derive(Resource)]
struct NetworkReceiver(Receiver<String>);

#[derive(Resource)]
struct NetworkSender(Sender<String>);

// --- L'ÉTAT DU JEU ---
#[derive(Resource)]
struct GameState {
    current_room: String,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            current_room: "unknown".to_string(),
        }
    }
}

// --- LES COMPOSANTS UI ---
#[derive(Component)]
struct ChatText;

#[derive(Component)]
struct InputText;

#[derive(Component)]
struct ChatUiRoot;

#[derive(Resource, Default)]
pub struct ChatConsole {
    pub open: bool,
    just_opened: bool,
}

fn parse_server_messages(
    mut events: EventReader<ServerMessageEvent>,
    mut game_state: ResMut<GameState>,
) {
    for ev in events.read() {
        let msg = &ev.0;

        let motif_salle = "room-loc.";
        if let Some(index_depart) = msg.find(motif_salle) {
            let debut_mot = index_depart + motif_salle.len();
            let mut fin_mot = debut_mot;

            for c in msg[debut_mot..].chars() {
                if c.is_whitespace() || c == '\n' || c == '\r' {
                    break;
                }
                fin_mot += c.len_utf8();
            }

            let room_id = &msg[debut_mot..fin_mot]; 
            if game_state.current_room != room_id {
                game_state.current_room = room_id.to_string();
                println!("[PARSER] 🗺️ Movement detected! New zone stored: {}", game_state.current_room);
            }
        }
    }
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/../sprites").to_string(),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(map::MapPlugin)
        .add_plugins(player::PlayerPlugin)
        .add_event::<ServerMessageEvent>()
        .init_resource::<GameState>()
        .init_resource::<ChatConsole>()
        .add_systems(Startup, (setup_network, setup_ui))
        .add_systems(Update, (
            read_network_messages,
            toggle_chat,
            handle_inputs.after(toggle_chat),
            update_chat_ui,
            parse_server_messages
        ))
        .run();
}

fn setup_ui(mut commands: Commands) {
    commands.spawn((
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
        parent.spawn(NodeBundle {
            style: Style {
                width: Val::Px(600.0),
                height: Val::Px(250.0),
                margin: UiRect { left: Val::Px(15.0), bottom: Val::Px(5.0), top: Val::Px(0.0), right: Val::Px(0.0) },
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
        parent.spawn(NodeBundle {
            style: Style {
                width: Val::Px(600.0),
                height: Val::Px(40.0),
                margin: UiRect { left: Val::Px(15.0), bottom: Val::Px(15.0), top: Val::Px(0.0), right: Val::Px(0.0) },
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

fn setup_network(mut commands: Commands) {
    let (tx_to_bevy, rx_in_bevy) = bounded::<String>(100);
    let (tx_to_server, rx_in_tokio) = bounded::<String>(100);

    commands.insert_resource(NetworkReceiver(rx_in_bevy));
    commands.insert_resource(NetworkSender(tx_to_server));

    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Ok(mut stream) = TcpStream::connect("127.0.0.1:4243").await {
                println!("[NETWORK] Connected to the Ombreval server!");
                let _ = tx_to_bevy.send("Network system initialized.".to_string());

                let (reader, mut writer) = stream.into_split();
                let mut buf_reader = BufReader::new(reader);

                let tx_clone = tx_to_bevy.clone();
                tokio::spawn(async move {
                    let mut line = String::new();
                    loop {
                        line.clear();
                        match buf_reader.read_line(&mut line).await {
                            Ok(0) => {
                                let _ = tx_clone.send("S: ERR Connection lost with the server.".to_string());
                                break;
                            }
                            Ok(_) => {
                                // On transmet la ligne nettoyée à Bevy
                                let _ = tx_clone.send(line.trim().to_string());
                            }
                            Err(_) => break,
                        }
                    }
                });

                loop {
                    while let Ok(message) = rx_in_tokio.try_recv() {
                        if writer.write_all(message.as_bytes()).await.is_err() {
                            return; 
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            } else {
                println!("[NETWORK] Unable to reach the server.");
            }
        });
    });
}

fn read_network_messages(
    receiver: Res<NetworkReceiver>,
    mut events: EventWriter<ServerMessageEvent>,
) {
    while let Ok(message) = receiver.0.try_recv() {
        println!("[BEVY RECEIVED]: {}", message);
        events.send(ServerMessageEvent(message));
    }
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

    if let Ok(mut text) = query.get_single_mut() {
        for ev in char_evr.read() {
            if swallow_chars {
                continue;
            }
            let s = ev.char.to_string();
            if !s.contains('\u{8}') && !s.contains('\r') && !s.contains('\n') {
                text.sections[0].value.push_str(&s);
            }
        }

        if keys.just_pressed(KeyCode::Backspace) {
            let mut current_text = text.sections[0].value.clone();
            if current_text.chars().count() > 2 { 
                current_text.pop();
                text.sections[0].value = current_text;
            }
        }

        if keys.just_pressed(KeyCode::Enter) {
            let current_text = text.sections[0].value.clone();
            let command = current_text[2..].trim(); 
            
            if !command.is_empty() {
                // On envoie au serveur avec un saut de ligne
                let _ = sender.0.send(format!("{}\n", command)); 
                // On vide le champ de saisie
                text.sections[0].value = "> ".to_string(); 
            }
        }
    }
}

fn update_chat_ui(
    mut events: EventReader<ServerMessageEvent>,
    mut query: Query<&mut Text, With<ChatText>>,
) {
    for ev in events.read() {
        for mut text in query.iter_mut() {
            text.sections[0].value.push_str(&format!("{}\n", ev.0));
            
            let lines: Vec<&str> = text.sections[0].value.lines().collect();
            if lines.len() > 8 {
                text.sections[0].value = lines[lines.len() - 8..].join("\n") + "\n";
            }
        }
    }
}