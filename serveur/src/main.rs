// serveur/src/main.rs

use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};
use serde_json::json;
use chrono::Local;
use std::sync::atomic::{AtomicBool, Ordering};
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Duration, Instant};
use reqwest::Client;

mod commands; 
mod world; 
mod state;
pub mod handlers;

use commands::GameCommand;
use world::WorldData;
use state::ServerState;
use rand::Rng;

#[derive(Clone, Debug)]
pub struct GlobalEvent {
    sender_addr: std::net::SocketAddr,
    message: String,
}

static LOG_MODE: AtomicBool = AtomicBool::new(false);

pub fn log_event(event_type: &str, player: &str, details: serde_json::Value) {
    if LOG_MODE.load(Ordering::SeqCst) {
        let log_entry = json!({
            "timestamp": Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
            "event": event_type,
            "player": player,
            "details": details
        });
        
        let log_string = log_entry.to_string();
        println!("{}", log_string);
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("logs.json") 
        {
            let _ = writeln!(file, "{}", log_string);
        }
    }
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|arg| arg == "--logs") {
        LOG_MODE.store(true, Ordering::SeqCst);
        println!("[SERVEUR] 🛠️  Mode LOG JSON activé.");
    }

    println!("[SERVEUR] Chargement de la carte...");
    let world_data = WorldData::load_from_file("world.yaml").expect("Erreur world.yaml");
    let shared_world = Arc::new(world_data);

    let mut initial_state = ServerState::new();
    initial_state.initialize_from_world(&shared_world);

    let shared_state = Arc::new(Mutex::new(initial_state));

    let (tx, _rx) = broadcast::channel::<GlobalEvent>(32);
    let shared_tx = Arc::new(tx);

    let listener = TcpListener::bind("127.0.0.1:4243").await?;
    println!("[SERVEUR] En écoute sur le port 4243... Système de Quêtes activé.");

    loop {
        let (mut socket, addr) = listener.accept().await?;
        
        let world = Arc::clone(&shared_world);
        let state = Arc::clone(&shared_state);
        let tx = Arc::clone(&shared_tx);
        let mut rx = tx.subscribe();

        tokio::spawn(async move {
            let (reader, mut writer) = socket.split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            let _ = writer.write_all(b"S: DK hello proto=1\n").await;

            loop {
                line.clear();
                
                tokio::select! {
                    result = reader.read_line(&mut line) => {
                        match result {
                            Ok(_) => {
                                let commande_analysee = GameCommand::parse(&line);

                                let (reponse, client_veut_quitter) = {
                                    let mut guard = state.lock().await;
                                    let server_state = &mut *guard;

                                    server_state.update_respawns(&world);
                                    
                                    crate::handlers::process_command(
                                        addr, 
                                        commande_analysee, 
                                        server_state, 
                                        &world, 
                                        &tx
                                    )
                                };

                                if writer.write_all(reponse.as_bytes()).await.is_err() { break; }

                                if client_veut_quitter {
                                    let mut guard = state.lock().await;
                                    let server_state = &mut *guard;
                                    if let Some(player) = server_state.remove_player(addr) {
                                        log_event("DISCONNECT", &player.username, json!({"reason": "QUIT command"}));
                                        let _ = tx.send(GlobalEvent { sender_addr: addr, message: format!("S: EVT GLOBAL CHAT Serveur {} a quitté le monde.\n", player.username) });
                                    }
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    Ok(event) = rx.recv() => {
                        if event.sender_addr != addr {
                            let _ = writer.write_all(event.message.as_bytes()).await;
                        }
                    }
                }
            }
        });
    }
}