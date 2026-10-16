use bevy::prelude::*;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::thread;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

const SERVER_ADDR: &str = "127.0.0.1:4243";

#[derive(Event)]
pub struct ServerMessageEvent(pub String);

#[derive(Resource)]
struct NetworkReceiver(Receiver<String>);

#[derive(Resource)]
pub struct NetworkSender(pub Sender<String>);

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ServerMessageEvent>()
            .add_systems(Startup, setup_network)
            .add_systems(PreUpdate, read_network_messages);
    }
}

fn setup_network(mut commands: Commands) {
    let (tx_to_bevy, rx_in_bevy) = bounded::<String>(100);
    let (tx_to_server, rx_in_tokio) = bounded::<String>(100);

    commands.insert_resource(NetworkReceiver(rx_in_bevy));
    commands.insert_resource(NetworkSender(tx_to_server));

    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let Ok(stream) = TcpStream::connect(SERVER_ADDR).await else {
                println!("[NETWORK] Unable to reach the server.");
                return;
            };
            println!("[NETWORK] Connected to the server");
            let _ = tx_to_bevy.send("Network system initialized.".to_string());

            let (reader, mut writer) = stream.into_split();
            let mut buf_reader = BufReader::new(reader);

            tokio::spawn(async move {
                let mut line = String::new();
                loop {
                    line.clear();
                    match buf_reader.read_line(&mut line).await {
                        Ok(0) => {
                            let _ = tx_to_bevy.send("S: ERR Connection lost with the server.".to_string());
                            break;
                        }
                        Ok(_) => { let _ = tx_to_bevy.send(line.trim().to_string()); }
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
        });
    });
}

fn read_network_messages(receiver: Res<NetworkReceiver>, mut events: EventWriter<ServerMessageEvent>) {
    while let Ok(message) = receiver.0.try_recv() {
        if !message.contains(" POS ") {
            println!("[BEVY RECEIVED]: {}", message);
        }
        events.send(ServerMessageEvent(message));
    }
}
