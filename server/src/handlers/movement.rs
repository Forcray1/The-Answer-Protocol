//! `LOOK`, `POS` and `MOVE`.

use std::net::SocketAddr;
use std::time::{Duration, Instant};
use serde_json::json;
use tokio::sync::broadcast::Sender;

use domain::{Direction, ItemId};

use crate::state::ServerState;
use crate::world::WorldData;
use crate::events::{to_room, GlobalEvent};
use crate::handlers::quest_core::quest_item_collected;
use crate::handlers::wire::{presence_roster_line, room_change_snapshot, room_presence_roster};

pub(crate) fn handle_look(addr: SocketAddr, state: &ServerState, world: &WorldData) -> String {
    let Some(player) = state.players.get(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    let Some(loc) = world.world.locations.get(&player.current_room) else {
        return "S: ERR room_not_found\n".to_string();
    };

    // Items on the ground (quest items already collected by this player are hidden).
    let items: Vec<String> = state
        .room_items
        .get(&player.current_room)
        .map(|ground| {
            ground
                .iter()
                .filter(|ri| {
                    world.item_by_id(&ri.item_id).is_some()
                        && !quest_item_collected(player, ri, world)
                })
                .map(|ri| ri.item_id.to_string())
                .collect()
        })
        .unwrap_or_default();

    let npcs: Vec<String> = state
        .room_npcs
        .get(&player.current_room)
        .map(|ids| ids.iter().map(|id| id.to_string()).collect())
        .unwrap_or_default();

    let players: Vec<&str> = state
        .players
        .values()
        .filter(|p| p.current_room == player.current_room)
        .map(|p| p.username.as_str())
        .collect();

    let mut exits: Vec<&str> = loc.exits.keys().map(|d| d.as_str()).collect();
    exits.sort_unstable();

    // RFC 42TAP: OK { "room": {...}, "players": [...], "items": [...], "npcs": [...] }
    let reply = json!({
        "room": {
            "id": player.current_room.as_str(),
            "name": loc.name,
            "description": loc.description,
            "exits": exits,
        },
        "players": players,
        "items": items,
        "npcs": npcs,
    });
    format!("S: OK {}\n", reply)
}

pub(crate) fn handle_pos(
    addr: SocketAddr,
    x: f32,
    y: f32,
    state: &mut ServerState,
    tx: &Sender<GlobalEvent>,
) -> String {
    if let Some(player) = state.players.get_mut(&addr) {
        player.pos_x = x;
        player.pos_y = y;
        let room = player.current_room.clone();
        let name = player.username.clone();
        to_room(
            tx,
            Some(addr),
            format!("S: EVT ROOM POS {} {} {}\n", name, x as i64, y as i64),
            room,
        );
    }
    String::new()
}

pub(crate) fn handle_move(
    addr: SocketAddr,
    dir: String,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    let (old_room, next_room, username, skin, px, py) = {
        let Some(player) = state.players.get_mut(&addr) else {
            return "S: ERR utilize_connect_first\n".to_string();
        };
        if player
            .last_move
            .is_some_and(|last| last.elapsed() < Duration::from_millis(500)) {
            return "S: ERR movement_cooldown_too_fast\n".to_string();
        }
        let direction = match Direction::parse(&dir) {
            Some(d) => d,
            None => return format!("S: ERR unknown_direction {}\n", dir),
        };
        let next_room = match world.world.locations.get(&player.current_room) {
            Some(loc) => match loc.exits.get(&direction) {
                Some(n) => n.clone(),
                None => return format!("S: ERR no exit to the {}\n", dir),
            },
            None => return "S: ERR room_error\n".to_string(),
        };

        if player.current_room.as_str() == "Castle"
            && direction == Direction::East
            && next_room.as_str() == "Temple1"
            && !player.inventory.contains(&ItemId::from("temple_pass")) {
            return "S: ERR temple_pass_required\n".to_string();
        }

        let old_room = player.current_room.clone();
        let username = player.username.clone();
        let skin = player.skin.clone();
        let (px, py) = (player.pos_x, player.pos_y);
        player.current_room = next_room.clone();
        player.last_move = Some(Instant::now());
        (old_room, next_room, username, skin, px, py)
    };

    to_room(
        tx,
        Some(addr),
        format!("S: EVT ROOM PRESENCE LEAVE {}\n", username),
        old_room.clone(),
    );
    to_room(
        tx,
        Some(addr),
        format!("S: EVT ROOM PRESENCE ENTER {}\n", username),
        next_room.clone(),
    );
    to_room(
        tx,
        Some(addr),
        presence_roster_line(&username, &skin, px as i64, py as i64),
        next_room.clone(),
    );
    format!(
        "{}{}",
        room_change_snapshot(addr, &next_room, state, world),
        room_presence_roster(addr, state, &next_room)
    )
}
