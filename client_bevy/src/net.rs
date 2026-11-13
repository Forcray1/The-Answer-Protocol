use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded};
use std::thread;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::AppState;
use crate::menu::LoginData;

const SERVER_ADDR: &str = "127.0.0.1:4243";

/// Seconds between two connection attempts while the login screen is shown
/// and the server is not connected.
const RECONNECT_INTERVAL: f32 = 1.5;

// Messages produced by the network thread itself (they are not sent by the server).
const MSG_CONNECTED: &str = "Network system initialized.";
const MSG_UNREACHABLE: &str = "S: ERR Unable to reach the server.";
const MSG_LOST: &str = "S: ERR Connection lost with the server.";
/// The connection was closed after the server said goodbye (normal QUIT).
const MSG_CLOSED: &str = "S: INFO Connection closed.";

#[derive(Event)]
pub struct ServerMessageEvent(pub String);

#[derive(Resource)]
struct NetworkReceiver(Receiver<String>);

#[derive(Resource)]
pub struct NetworkSender(pub Sender<String>);

/// State of the TCP connection, used by the login screen and the auto-reconnect.
#[derive(Resource, Default)]
pub struct NetworkStatus {
    pub connected: bool,
    attempting: bool,
}

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ServerMessageEvent>()
            .init_resource::<NetworkStatus>()
            .add_systems(Startup, setup_network)
            .add_systems(PreUpdate, read_network_messages)
            .add_systems(
                Update,
                (
                    return_to_login_on_disconnect,
                    retry_connection_on_login_screen,
                ),
            );
    }
}

fn setup_network(mut commands: Commands, mut status: ResMut<NetworkStatus>) {
    status.attempting = true;
    spawn_connection(&mut commands);
}

/// Opens a new TCP connection on a background thread and (re)creates the channels
/// used to talk to it. Replacing the resources drops the previous connection's channels,
/// which makes the previous thread stop.
fn spawn_connection(commands: &mut Commands) {
    let (tx_to_bevy, rx_in_bevy) = bounded::<String>(100);
    let (tx_to_server, rx_in_tokio) = bounded::<String>(100);

    commands.insert_resource(NetworkReceiver(rx_in_bevy));
    commands.insert_resource(NetworkSender(tx_to_server));

    thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("[NETWORK] Failed to create async runtime: {}", e);
                let _ = tx_to_bevy.send(MSG_UNREACHABLE.to_string());
                return;
            }
        };
        rt.block_on(async move {
            let Ok(stream) = TcpStream::connect(SERVER_ADDR).await else {
                println!("[NETWORK] Unable to reach the server.");
                let _ = tx_to_bevy.send(MSG_UNREACHABLE.to_string());
                return;
            };
            println!("[NETWORK] Connected to the server");
            let _ = tx_to_bevy.send(MSG_CONNECTED.to_string());

            let (reader, mut writer) = stream.into_split();
            let mut buf_reader = BufReader::new(reader);

            tokio::spawn(async move {
                let mut line = String::new();
                let mut said_goodbye = false;
                loop {
                    line.clear();
                    match buf_reader.read_line(&mut line).await {
                        Ok(0) | Err(_) => {
                            // After "OK goodbye" the close is expected; otherwise it is an error.
                            let msg = if said_goodbye { MSG_CLOSED } else { MSG_LOST };
                            let _ = tx_to_bevy.send(msg.to_string());
                            break;
                        }
                        Ok(_) => {
                            let text = line.trim().to_string();
                            if text == "S: OK goodbye" {
                                said_goodbye = true;
                            }
                            let _ = tx_to_bevy.send(text);
                        }
                    }
                }
            });

            loop {
                loop {
                    match rx_in_tokio.try_recv() {
                        Ok(message) => {
                            if writer.write_all(message.as_bytes()).await.is_err() {
                                return;
                            }
                        }
                        Err(TryRecvError::Empty) => break,
                        // The Bevy side replaced/dropped its sender: this connection is obsolete.
                        Err(TryRecvError::Disconnected) => return,
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });
    });
}

fn read_network_messages(
    receiver: Res<NetworkReceiver>,
    mut events: EventWriter<ServerMessageEvent>,
    mut status: ResMut<NetworkStatus>,
) {
    while let Ok(message) = receiver.0.try_recv() {
        match message.as_str() {
            MSG_CONNECTED => {
                status.connected = true;
                status.attempting = false;
            }
            MSG_UNREACHABLE => {
                status.connected = false;
                status.attempting = false;
            }
            MSG_LOST | MSG_CLOSED => {
                status.connected = false;
            }
            _ => {}
        }
        if !message.contains(" POS ") {
            println!("[BEVY RECEIVED]: {}", message);
        }
        events.send(ServerMessageEvent(message));
    }
}

/// QUIT (`S: OK goodbye`) or a dropped connection while playing: back to the login screen.
fn return_to_login_on_disconnect(
    mut events: EventReader<ServerMessageEvent>,
    state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut login: ResMut<LoginData>,
) {
    for ev in events.read() {
        if *state.get() != AppState::InGame {
            continue;
        }
        let msg = ev.0.as_str();
        if msg == "S: OK goodbye" {
            login.error_msg = None;
        } else if msg == MSG_LOST {
            login.error_msg = Some("Connection lost with the server.".to_string());
        } else {
            continue;
        }
        login.password.clear();
        next_state.set(AppState::MainMenu);
    }
}

/// On the login screen, reconnect automatically whenever the server is not connected
/// (after a QUIT, a server restart, or when the GUI was started before the server).
fn retry_connection_on_login_screen(
    time: Res<Time>,
    state: Res<State<AppState>>,
    mut status: ResMut<NetworkStatus>,
    mut commands: Commands,
    mut elapsed: Local<f32>,
) {
    if *state.get() != AppState::MainMenu || status.connected || status.attempting {
        *elapsed = 0.0;
        return;
    }
    *elapsed += time.delta_seconds();
    if *elapsed >= RECONNECT_INTERVAL {
        *elapsed = 0.0;
        status.attempting = true;
        println!("[NETWORK] Trying to connect...");
        spawn_connection(&mut commands);
    }
}
