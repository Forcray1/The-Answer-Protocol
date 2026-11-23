//! One task per TCP client: reads command lines, answers them and forwards broadcast events.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::TcpStream;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::{mpsc, Mutex};

use crate::commands::GameCommand;
use crate::events::{to_all, to_room, GlobalEvent};
use crate::handlers::{process_command, process_connect, save_player_to_db};
use crate::logging::log_event;
use crate::state::ServerState;
use crate::world::WorldData;
use crate::DbPool;

/// Logs `ABUSE_PATTERN` when one IP opens 5+ connections within 10 seconds.
pub fn note_connection(
    recent: &mut HashMap<String, VecDeque<Instant>>,
    ip: &str,
    now: Instant,
) {
    let entries = recent.entry(ip.to_string()).or_default();
    while entries
        .front()
        .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(10))
    {
        entries.pop_front();
    }
    entries.push_back(now);
    if entries.len() >= 5 {
        log_event(
            "ABUSE_PATTERN",
            ip,
            json!({
                "type": "rapid_connections",
                "count": entries.len(),
                "window_seconds": 10
            }),
        );
    }
}

/// Reads lines in a background task so that `select!` below never cancels a half-read line.
fn spawn_line_reader(reader: OwnedReadHalf) -> mpsc::UnboundedReceiver<String> {
    let (line_tx, line_rx) = mpsc::unbounded_channel::<String>();
    let mut reader = BufReader::new(reader);
    tokio::spawn(async move {
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    let trimmed = line.trim_end_matches(['\r', '\n']).to_string();
                    if line_tx.send(trimmed).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    line_rx
}

/// Logs `ABUSE_PATTERN` when a client sends 25+ commands within 2 seconds (at most every 5 s).
fn note_command(
    recent: &mut VecDeque<Instant>,
    last_flood_log: &mut Option<Instant>,
    player: Option<&str>,
    addr: SocketAddr,
) {
    let now = Instant::now();
    while recent
        .front()
        .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(2))
    {
        recent.pop_front();
    }
    recent.push_back(now);
    let should_log = recent.len() >= 25
        && last_flood_log.is_none_or(|t| now.duration_since(t) > Duration::from_secs(5));
    if should_log {
        log_event(
            "ABUSE_PATTERN",
            player.unwrap_or("anonymous"),
            json!({
                "type": "command_flooding",
                "addr": addr.to_string(),
                "commands_in_2s": recent.len()
            }),
        );
        *last_flood_log = Some(now);
    }
}

/// Removes the player, saves it and tells the others. `chat_suffix` ends the global chat line.
async fn disconnect(
    addr: SocketAddr,
    reason: &str,
    chat_suffix: &str,
    state: &Mutex<ServerState>,
    pool: &DbPool,
    tx: &Sender<GlobalEvent>,
) {
    let removed_player = {
        let mut guard = state.lock().await;
        guard.remove_player(addr)
    };
    let Some(player) = removed_player else {
        return;
    };
    save_player_to_db(pool, &player).await;
    log_event("DISCONNECT", &player.username, json!({"reason": reason}));
    to_room(
        tx,
        Some(addr),
        format!("S: EVT ROOM PRESENCE LEAVE {}\n", player.username),
        player.current_room.clone(),
    );
    to_all(
        tx,
        Some(addr),
        format!("S: EVT GLOBAL CHAT Server {} {}\n", player.username, chat_suffix),
    );
}

/// Does this connection have to receive a broadcast event?
/// `target_player` wins over `target_room`, which wins over `target_group`.
async fn wants_event(event: &GlobalEvent, addr: SocketAddr, state: &Mutex<ServerState>) -> bool {
    if event.sender_addr == Some(addr) {
        return false;
    }
    if let Some(target) = &event.target_player {
        return addr == *target;
    }
    if let Some(target) = &event.target_room {
        let guard = state.lock().await;
        return guard
            .players
            .get(&addr)
            .is_some_and(|p| &p.current_room == target);
    }
    if let Some(target) = &event.target_group {
        let guard = state.lock().await;
        return guard
            .players
            .get(&addr)
            .is_some_and(|p| p.group.as_ref() == Some(target));
    }
    true
}

/// Parses and runs one command line. Returns the reply and whether the client quits.
async fn run_line(
    line: &str,
    player_name: Option<&str>,
    addr: SocketAddr,
    state: &Arc<Mutex<ServerState>>,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
    pool: &DbPool,
) -> (String, bool) {
    let command = GameCommand::parse(line);
    match player_name {
        Some(name) => log_event("COMMAND", name, json!({"raw": line})),
        None => log_event(
            "COMMAND",
            "anonymous",
            json!({"addr": addr.to_string(), "raw": line}),
        ),
    }
    let (response, should_quit) = match command {
        GameCommand::Connect { username, password } => {
            let response =
                process_connect(addr, username, password, state, world, tx, pool).await;
            (response, false)
        }
        other => {
            let mut guard = state.lock().await;
            process_command(addr, other, &mut guard, world, tx)
        }
    };
    log_event(
        "RESPONSE",
        player_name.unwrap_or("anonymous"),
        json!({"raw": line, "response": response}),
    );
    (response, should_quit)
}

pub async fn handle_client(
    socket: TcpStream,
    addr: SocketAddr,
    state: Arc<Mutex<ServerState>>,
    world: Arc<WorldData>,
    tx: Arc<Sender<GlobalEvent>>,
    pool: DbPool,
    mut rx: Receiver<GlobalEvent>,
) {
    // Send small messages right away instead of letting TCP group them.
    let _ = socket.set_nodelay(true);
    let (reader, mut writer) = socket.into_split();
    let mut line_rx = spawn_line_reader(reader);
    let mut recent_commands: VecDeque<Instant> = VecDeque::new();
    let mut last_flood_log: Option<Instant> = None;

    let _ = writer.write_all(b"S: OK hello proto=1\n").await;

    loop {
        tokio::select! {
            maybe_line = line_rx.recv() => {
                let Some(line) = maybe_line else {
                    disconnect(addr, "connection_lost", "lost connection.", &state, &pool, &tx).await;
                    break;
                };

                let player_name = {
                    let guard = state.lock().await;
                    guard.players.get(&addr).map(|p| p.username.clone())
                };
                note_command(&mut recent_commands, &mut last_flood_log, player_name.as_deref(), addr);

                let (response, should_quit) =
                    run_line(&line, player_name.as_deref(), addr, &state, &world, &tx, &pool).await;

                if writer.write_all(response.as_bytes()).await.is_err() {
                    break;
                }
                if should_quit {
                    disconnect(addr, "QUIT", "left the world.", &state, &pool, &tx).await;
                    break;
                }
            }
            received = rx.recv() => {
                match received {
                    Ok(event) => {
                        if wants_event(&event, addr, &state).await {
                            let _ = writer.write_all(event.message.as_bytes()).await;
                        }
                    }
                    // Too far behind: skip what was lost and keep listening.
                    Err(RecvError::Lagged(missed)) => {
                        eprintln!("[NETWORK] {} missed {} broadcast events.", addr, missed);
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        }
    }
}
