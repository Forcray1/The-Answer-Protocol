//! Shared results of a fight: monster death, loot, victory reply, player death and respawn.

use std::net::SocketAddr;
use std::time::{Duration, Instant};
use serde_json::json;
use tokio::sync::broadcast::Sender;
use rand::Rng;

use domain::{CombatState, ItemBucket, RoomId};

use crate::state::{Player, ServerState, START_ROOM};
use crate::world::WorldData;
use crate::events::{to_player_from, to_room, to_all, GlobalEvent};
use crate::logging::log_event;
use crate::handlers::board::process_board_quest_kill;
use crate::handlers::player_events::player_stats_event;
use crate::handlers::wire::presence_roster_line;

#[allow(dead_code)]
pub(crate) fn respawn_player_at_start(player: &mut crate::state::Player) {
    let respawn_hp = (player.max_hp * 30 / 100).max(1);
    player.hp = respawn_hp;
    player.current_room = RoomId::from(START_ROOM);
    player.pos_x = crate::state::DEATH_RESPAWN_X;
    player.pos_y = crate::state::DEATH_RESPAWN_Y;
    player.stunned_turns = 0;
    player.combat = CombatState::Idle;
}

pub(crate) fn notify_player_respawn(
    tx: &Sender<GlobalEvent>,
    addr: SocketAddr,
    old_room: RoomId,
    username: &str,
    skin: &str,
) {
    to_room(
        tx,
        Some(addr),
        format!("S: EVT ROOM PRESENCE LEAVE {}\n", username),
        old_room,
    );
    to_room(
        tx,
        Some(addr),
        format!("S: EVT ROOM PRESENCE ENTER {}\n", username),
        RoomId::from(START_ROOM),
    );
    to_room(
        tx,
        Some(addr),
        presence_roster_line(
            username,
            skin,
            crate::state::DEATH_RESPAWN_X as i64,
            crate::state::DEATH_RESPAWN_Y as i64,
        ),
        RoomId::from(START_ROOM),
    );
    let _ = tx.send(GlobalEvent {
        sender_addr: Some(addr),
        message: "S: OK room=loc.Start_oasis\n".to_string(),
        target_room: Some(RoomId::from(START_ROOM)),
        target_group: None,
        target_player: Some(addr),
    });
    to_player_from(
        tx,
        addr,
        "S: OK Player got back to spawn after dying.\n".to_string(),
        addr,
    );
    to_room(
        tx,
        Some(addr),
        format!(
            "S: EVT ROOM POS {} {} {}\n",
            username, crate::state::DEATH_RESPAWN_X as i64, crate::state::DEATH_RESPAWN_Y as i64
        ),
        RoomId::from(START_ROOM),
    );
}

/// Respawns a player killed by `monster_name` at the start room and notifies everybody.
/// Returns the player's refreshed `EVT PLAYER_STATS`.
pub(crate) fn kill_player(
    addr: SocketAddr,
    player_name: &str,
    monster_name: &str,
    state: &mut ServerState,
    tx: &Sender<GlobalEvent>,
) -> String {
    state.active_combats.remove(&addr);
    let (username, skin, old_room, final_stats) = if let Some(player) = state.players.get_mut(&addr) {
        let username = player.username.clone();
        let skin = player.skin.clone();
        let old_room = player.current_room.clone();
        respawn_player_at_start(player);
        let final_stats = if let Some(player) = state.players.get(&addr) {
            player_stats_event(player)
        } else {
            String::new()
        };
        (username, skin, old_room, final_stats)
    } else {
        (String::new(), String::new(), RoomId::from(START_ROOM), String::new())
    };
    if !username.is_empty() {
        notify_player_respawn(tx, addr, old_room, &username, &skin);
        to_player_from(
            tx,
            addr,
            final_stats.clone(),
            addr,
        );
    }
    to_all(
        tx,
        Some(addr),
        format!(
            "S: EVT GLOBAL CHAT Server A player was killed by {}!\n",
            monster_name
        ),
    );
    log_event("DEATH", &player_name, json!({"killer": monster_name}));
    final_stats
}

/// Ends the fight against a monster that just died and schedules its respawn.
pub(crate) fn remove_dead_monster(
    addr: SocketAddr,
    m_id: &domain::NpcId,
    room: &RoomId,
    state: &mut ServerState,
    world: &WorldData,
) {
    if let Some(player) = state.players.get_mut(&addr) {
        player.combat = CombatState::Idle;
    }
    state.active_combats.remove(&addr);
    if let Some(npcs_in_room) = state.room_npcs.get_mut(room) {
        npcs_in_room.retain(|id| id != m_id);
    }
    if let Some(npc_static) = world.npc_by_id(m_id) {
        let delay = npc_static.respawn_time.unwrap_or(30);
        state.dead_npcs.push(crate::state::DeadNpc {
            npc_id: m_id.clone(),
            room_id: room.clone(),
            respawn_at: Instant::now() + Duration::from_secs(delay),
        });
    }
}

/// Rolls the loot table of a dead monster into the player's inventory.
/// Returns the display names of what dropped.
pub(crate) fn roll_drops(
    player: &mut Player,
    drops: Vec<crate::world::DropChance>,
    world: &WorldData,
) -> Vec<String> {
    let mut drop_names = Vec::new();
    for drop in drops {
        if rand::thread_rng().gen_range(1..=100) <= drop.chance {
            let bucket = world.item_by_id(&drop.item_id)
                .map(|i| i.bucket())
                .unwrap_or(ItemBucket::Item);
            player.inventory.add(drop.item_id.clone(), bucket);
            let nom = world.item_by_id(&drop.item_id)
                .map(|i| i.name.clone())
                .unwrap_or_else(|| drop.item_id.to_string());
            drop_names.push(nom);
        }
    }
    drop_names
}

/// Final step of a won fight: despawns the mob for the room, settles the board quest
/// and builds the reply. `headline` is the first sentence ("You dealt 12 damage").
#[allow(clippy::too_many_arguments)]
pub(crate) fn announce_victory(
    addr: SocketAddr,
    player_name: &str,
    m_id: &domain::NpcId,
    room: &RoomId,
    headline: String,
    monster_name: &str,
    exp_gained: i32,
    drop_names: &[String],
    mut stat_str: String,
    state: &mut ServerState,
    tx: &Sender<GlobalEvent>,
) -> String {
    to_room(
        tx,
        None,
        format!("S: EVT ROOM MOB_DESPAWN {}\n", m_id),
        room.clone(),
    );
    let board_bonus = process_board_quest_kill(player_name, m_id.as_str(), state);
    // If a board quest completed, refresh the HUD gold and remove it from the quest log.
    if !board_bonus.is_empty() {
        if let Some(p) = state.players.get(&addr) {
            stat_str.push_str(&format!("S: EVT GOLD {}\n", p.money));
            stat_str.push_str("S: EVT QUEST_DATA empty\n");
        }
    }
    let mut msg = format!(
        "S: EVT COMBAT END victory\nS: OK {}. {} collapses! (+{} EXP){}\n",
        headline, monster_name, exp_gained, board_bonus
    );
    if !drop_names.is_empty() {
        msg.push_str(&format!("You obtain: {}\n", drop_names.join(", ")));
    }
    msg.push_str(&stat_str);
    msg
}
