//! `CHAT`, `WHO` and `STATUS`.

use std::net::SocketAddr;
use std::time::{Duration, Instant};
use serde_json::json;
use tokio::sync::broadcast::Sender;

use domain::RoomId;

use crate::state::ServerState;
use crate::events::{to_room, to_all, GlobalEvent};

pub(crate) fn handle_chat(
    addr: SocketAddr,
    channel: String,
    message: String,
    state: &mut ServerState,
    tx: &Sender<GlobalEvent>,
) -> String {
    let mut on_cooldown = false;
    let mut current_room = String::new();
    if let Some(player) = state.players.get(&addr) {
        if player
            .last_chat
            .is_some_and(|last| last.elapsed() < Duration::from_millis(2000)) {
            on_cooldown = true;
        } else {
            current_room = player.current_room.as_str().to_string();
        }
    }
    if on_cooldown {
        return "S: ERR chat_spam_forbidden\n".to_string();
    }
    if let Some(player) = state.players.get_mut(&addr) {
        player.last_chat = Some(Instant::now());
        let username = player.username.clone();
        let mut group_id_opt = None;
        if channel.as_str() == "GROUP" {
            group_id_opt = player.group.clone();
            if group_id_opt.is_none() {
                return "S: ERR you_have_no_group\n".to_string();
            }
        }

        match channel.as_str() {
            "GLOBAL" => {
                to_all(
                    tx,
                    None,
                    format!("S: EVT GLOBAL CHAT {} {}\n", username, message),
                );
                "S: OK\n".to_string()
            }
            "ROOM" => {
                to_room(
                    tx,
                    None,
                    format!("S: EVT ROOM CHAT {} {}\n", username, message),
                    RoomId::from(current_room.as_str()),
                );
                "S: OK\n".to_string()
            }
            "GROUP" => {
                let _ = tx.send(GlobalEvent {
                    sender_addr: None,
                    message: format!("S: EVT GROUP CHAT {} {}\n", username, message),
                    target_room: None,
                    target_group: group_id_opt,
                    target_player: None,
                });
                "S: OK\n".to_string()
            }
            _ => "S: ERR unknown_channel\n".to_string(),
        }
    } else {
        "S: ERR utilize_connect_first\n".to_string()
    }
}

pub(crate) fn handle_who(addr: SocketAddr, state: &ServerState) -> String {
    let total_serveur = state.players.len();
    let Some(current_player) = state.players.get(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    let players_in_room: Vec<&str> = state
        .players
        .values()
        .filter(|p| p.current_room == current_player.current_room)
        .map(|p| p.username.as_str())
        .collect();
    // RFC 42TAP: OK { "room": ["alice", "bob"], "server": 5 }
    format!(
        "S: OK {}\n",
        json!({ "room": players_in_room, "server": total_serveur })
    )
}

pub(crate) fn handle_status(addr: SocketAddr, state: &ServerState) -> String {
    let Some(player) = state.players.get(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    format!(
        "S: OK HP: {}/{} | Lv: {} | XP: {}/{} | Gold: {} | Location: {}\n",
        player.hp,
        player.max_hp,
        player.level,
        player.xp_bar.current,
        player.xp_bar.requiered,
        player.money,
        player.current_room
    )
}
