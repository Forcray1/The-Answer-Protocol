use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};
use serde_json::json;
use chrono::Local;
use std::sync::atomic::{AtomicBool, Ordering};
use std::fs::OpenOptions;
use std::io::Write;

mod commands;
mod world;
mod state;
pub mod handlers;

use commands::GameCommand;
use world::WorldData;
use state::ServerState;

// Le pool SQLite est partagé entre tous les handlers via Arc
// SqlitePool est déjà thread-safe, pas besoin de Mutex autour
pub type DbPool = Arc<sqlx::SqlitePool>;

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
        println!("[SERVER] JSON log mode enabled.");
    }

    // 1. Initialise la BDD — crée game.db + tables si nécessaire
    println!("[SERVER] Initializing database...");
    let pool = database::init_db("sqlite://game.db").await?;
    let shared_pool: DbPool = Arc::new(pool);
    println!("[SERVER] Database ready.");

    // 2. Charge le monde depuis le YAML (inchangé)
    println!("[SERVER] Loading map...");
    let world_data = WorldData::load_from_file("world.yaml").expect("world.yaml error");
    let shared_world = Arc::new(world_data);

    // 3. Initialise l'état en mémoire (inchangé)
    let mut initial_state = ServerState::new();
    initial_state.initialize_from_world(&shared_world);
    let shared_state = Arc::new(Mutex::new(initial_state));

    let (tx, _rx) = broadcast::channel::<GlobalEvent>(32);
    let shared_tx = Arc::new(tx);

    let listener = TcpListener::bind("127.0.0.1:4243").await?;
    println!("[SERVER] Listening on port 4243...");

    loop {
        let (mut socket, addr) = listener.accept().await?;

        let world = Arc::clone(&shared_world);
        let state = Arc::clone(&shared_state);
        let tx = Arc::clone(&shared_tx);
        let pool = Arc::clone(&shared_pool); // clone du Arc, pas du pool
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
                            Ok(0) | Err(_) => {
                                // Connexion fermée brutalement — sauvegarde quand même
                                let mut guard = state.lock().await;
                                if let Some(player) = guard.remove_player(addr) {
                                    handlers::save_player_to_db(&pool, &player).await;
                                    log_event("DISCONNECT", &player.username, json!({"reason": "connection_lost"}));
                                    let _ = tx.send(GlobalEvent {
                                        sender_addr: addr,
                                        message: format!("S: EVT GLOBAL CHAT Server {} lost connection.\n", player.username)
                                    });
                                }
                                break;
                            }
                            Ok(_) => {
                                let commande = GameCommand::parse(&line);

                                let (reponse, quitter) = {
                                    let mut guard = state.lock().await;
                                    let server_state = &mut *guard;
                                    server_state.update_respawns(&world);
                                    handlers::process_command(
                                        addr,
                                        commande,
                                        server_state,
                                        &world,
                                        &tx,
                                        &pool,
                                    ).await
                                };

                                if writer.write_all(reponse.as_bytes()).await.is_err() { break; }

                                if quitter {
                                    let mut guard = state.lock().await;
                                    if let Some(player) = guard.remove_player(addr) {
                                        handlers::save_player_to_db(&pool, &player).await;
                                        log_event("DISCONNECT", &player.username, json!({"reason": "QUIT"}));
                                        let _ = tx.send(GlobalEvent {
                                            sender_addr: addr,
                                            message: format!("S: EVT GLOBAL CHAT Server {} left the world.\n", player.username)
                                        });
                                    }
                                    break;
                                }
                            }
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