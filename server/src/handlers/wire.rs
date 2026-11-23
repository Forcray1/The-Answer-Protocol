//! Wire-format builders for room snapshots (exits, mobs, items, presence rosters).

use std::net::SocketAddr;

use domain::RoomId;

use crate::state::ServerState;
use crate::world::{Npc, WorldData};
use crate::handlers::quest_core::quest_item_collected;

pub(crate) fn format_exits(world: &WorldData, room: &RoomId) -> String {
    let mut exits_str = String::new();
    if let Some(loc) = world.world.locations.get(room) {
        let exits: Vec<&str> = loc.exits.keys().map(|d| d.as_str()).collect();
        if !exits.is_empty() {
            exits_str = format!(" exits={}", exits.join(","));
        }
    }
    exits_str
}

pub(crate) fn room_change_snapshot(
    addr: SocketAddr,
    room: &RoomId,
    state: &ServerState,
    world: &WorldData,
) -> String {
    let exits = format_exits(world, room);
    format!(
        "S: OK room=loc.{}{}\n{}{}",
        room,
        exits,
        room_mob_roster(room, state, world),
        room_item_roster(addr, room, state, world)
    )
}

/// Build the `MOB_SPAWN` / `BOARD_SPAWN` events for the NPCs visible in `room`.
pub(crate) fn room_mob_roster(room: &RoomId, state: &ServerState, world: &WorldData) -> String {
    let mut out = String::new();
    if let Some(npc_ids) = state.room_npcs.get(room) {
        for npc_id in npc_ids {
            if let Some(npc) = world.npc_by_id(npc_id) {
                let hp = state.npc_hps.get(npc_id).copied().unwrap_or(npc.hp);
                out.push_str(&npc_spawn_event(npc, hp));
            }
        }
    }
    out
}

pub(crate) fn presence_roster_line(username: &str, skin: &str, x: i64, y: i64) -> String {
    format!("S: EVT ROOM PRESENCE ROSTER {} {} {} {}\n", username, skin, x, y)
}

/// Build the `ITEM_SPAWN` events for the items on the ground in `room` that the player
/// at `addr` can see (quest items they already collected are hidden).
pub(crate) fn room_item_roster(
    addr: SocketAddr,
    room: &RoomId,
    state: &ServerState,
    world: &WorldData,
) -> String {
    let viewer = state.players.get(&addr);
    let mut out = String::new();
    if let Some(items_in_room) = state.room_items.get(room) {
        for ri in items_in_room {
            if viewer.is_some_and(|player| quest_item_collected(player, ri, world)) {
                continue;
            }
            if let Some(item) = world.item_by_id(&ri.item_id) {
                let sprite = item.sprite.as_deref().unwrap_or("none");
                out.push_str(&format!(
                    "S: EVT ROOM ITEM_SPAWN {} {} {} {} {} {}\n",
                    item.id,
                    item.name.replace(' ', "_"),
                    sprite,
                    ri.pos_x as i32,
                    ri.pos_y as i32,
                    ri.uid,
                ));
            }
        }
    }
    out
}

pub(crate) fn room_presence_roster(addr: SocketAddr, state: &ServerState, room: &RoomId) -> String {
    let mut out = String::new();
    for (other_addr, p) in &state.players {
        if *other_addr != addr && &p.current_room == room {
            out.push_str(&format!(
                "S: EVT ROOM PRESENCE ROSTER {} {} {} {}\n",
                p.username, p.skin, p.pos_x as i64, p.pos_y as i64
            ));
        }
    }
    out
}

/// Wire event announcing an NPC in its room: `BOARD_SPAWN` for board NPCs (no character
/// sprite on the client side), `MOB_SPAWN` for every other NPC. `hp` is its current hp.
pub fn npc_spawn_event(npc: &Npc, hp: i32) -> String {
    let (x, y) = npc
        .spawn_pos
        .map(|p| (p[0] as i64, p[1] as i64))
        .unwrap_or((0, 0));
    if npc.role == "board" {
        format!("S: EVT ROOM BOARD_SPAWN {} {} {}\n", npc.id, x, y)
    } else {
        format!(
            "S: EVT ROOM MOB_SPAWN {} {} {} {} {} {} {} {} {}\n",
            npc.id,
            npc.map_sprite(),
            x,
            y,
            hp,
            npc.hp,
            npc.scale.unwrap_or(1.0),
            npc.name.replace(' ', "_"),
            npc.role
        )
    }
}
