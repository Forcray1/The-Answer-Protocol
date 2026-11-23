//! `CONNECT` (login / registration / guest) and player persistence.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use serde_json::json;
use tokio::sync::broadcast::Sender;
use tokio::sync::Mutex;

use domain::{xp_required_for_level, ItemBucket, ItemId, RoomId};

use crate::state::{Player, ServerState, START_ROOM};
use crate::world::WorldData;
use crate::events::{to_room, to_all, GlobalEvent};
use crate::logging::log_event;
use crate::DbPool;
use crate::handlers::equip::build_equipement;
use crate::handlers::player_events::{get_char_stats_event_with_world, get_equip_data_event, get_inv_data_event};
use crate::handlers::wire::{format_exits, presence_roster_line, room_item_roster, room_mob_roster, room_presence_roster};

pub async fn save_player_to_db(pool: &DbPool, player: &Player) {
    // Guest sessions (CONNECT without password) are never persisted.
    if player.guest {
        return;
    }
    let mut equipment: HashMap<String, ItemId> = HashMap::new();
    for (slot, id, _name) in player.equipement.equipped() {
        equipment.insert(slot.to_string(), id);
    }
    // Persist the equipped medaillon. The saved inventory still counts it, which keeps
    // existing saves valid: loading removes exactly one copy from the inventory.
    let mut inventory = player.inventory.clone();
    if let Some(mid) = &player.equipped_medaillon_id {
        equipment.insert("medaillon".to_string(), mid.clone());
        inventory.add(mid.clone(), ItemBucket::Item);
    }

    let result = database::save_player(
        pool,
        &player.username,
        player.hp,
        player.xp_bar.current,
        player.level,
        player.money,
        player.current_room.as_str(),
        &inventory,
        &equipment,
        &player.completed_quests,
        &player.skin,
        player.pos_x as f64,
        player.pos_y as f64,
        player.stat_points,
        player.stats.damage,
        player.stats.defense,
        player.stats.special_damage,
        player.stats.special_defense,
        player.max_hp - player.base_max_hp, // extra hp from STATUP only
    )
    .await;

    if let Err(e) = result {
        eprintln!("[DB] Error saving player {}: {}", player.username, e);
    } else {
        println!("[DB] Player {} saved.", player.username);
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn process_connect(
    addr: SocketAddr,
    pseudo: String,
    password: String,
    state: &Arc<Mutex<ServerState>>,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
    pool: &DbPool,
) -> String {
    let guest = password.trim().is_empty();

    {
        let guard = state.lock().await;
        if guard.players.contains_key(&addr) {
            return "S: ERR you_are_already_connected\n".to_string();
        }
        if guard.players.values().any(|p| p.username == pseudo) {
            return "S: ERR already_logged_in\n".to_string();
        }
    }

    let loaded = if guest {
        None
    } else {
        let loaded = match database::load_player(pool, &pseudo).await {
            Ok(data) => data,
            Err(e) => {
                eprintln!("[DB] Player load error: {}", e);
                return "S: ERR internal_error\n".to_string();
            }
        };

        if loaded.is_some() {
            match database::verify_player(pool, &pseudo, &password).await {
                Ok(true) => {}
                Ok(false) => {
                    log_event("AUTH_FAIL", &pseudo, json!({"ip": addr.to_string()}));
                    return "S: ERR bad_credentials\n".to_string();
                }
                Err(e) => {
                    eprintln!("[DB] Verification error: {}", e);
                    return "S: ERR internal_error\n".to_string();
                }
            }
        } else if let Err(e) = database::register_player(pool, &pseudo, &password).await {
            eprintln!("[DB] Account creation error: {}", e);
            return "S: ERR internal_error\n".to_string();
        }
        loaded
    };

    let mut guard = state.lock().await;
    if guard.players.contains_key(&addr) {
        return "S: ERR you_are_already_connected\n".to_string();
    }
    if guard.players.values().any(|p| p.username == pseudo) {
        return "S: ERR already_logged_in\n".to_string();
    }

    if let Some(data) = loaded {
        let room = if world.world.locations.contains_key(&data.current_room) {
            data.current_room
        } else {
            eprintln!(
                "[CONNECT] Unknown room '{}' for player '{}', fallback to '{}'.",
                data.current_room, pseudo, START_ROOM
            );
            RoomId::from(START_ROOM)
        };

        let saved_pos = data.position;
        let mut player = Player::new(pseudo.clone(), room);
        player.hp = data.hp;
        player.level = data.level.max(1);
        player.xp_bar.current = data.exp;
        player.xp_bar.requiered = xp_required_for_level(player.level);
        player.money = data.money;
        player.inventory = data.inventory;
        player.completed_quests = data.completed_quests;
        player.equipement = build_equipement(world, &data.equipment);
        player.skin = data.skin;
        player.stat_points = data.stat_points;
        player.stats.damage = data.base_damage;
        player.stats.defense = data.base_defense;
        player.stats.special_damage = data.base_spe_dmg;
        player.stats.special_defense = data.base_spe_def;
        player.max_hp = player.base_max_hp + data.base_max_hp;
        player.hp = player.hp.min(player.max_hp);
        if player.level > 1
            && data.stat_points == 0
            && data.base_damage == 0
            && data.base_defense == 0
            && data.base_spe_dmg == 0
            && data.base_spe_def == 0
            && data.base_max_hp == 0 {
            player.stat_points = player.level * 3;
        }
        if let Some(mid) = data.equipment.get("medaillon") {
            player.inventory.remove_one(mid);
            player.equipped_medaillon_id = Some(mid.clone());
        }
        if let Some((x, y)) = saved_pos {
            player.pos_x = x;
            player.pos_y = y;
        }

        let skin = player.skin.clone();
        let enter_room = player.current_room.clone();
        let (px, py) = (player.pos_x as i64, player.pos_y as i64);
        guard.players.insert(addr, player);

        let Some(p) = guard.players.get(&addr) else {
            return "S: ERR internal_error\n".to_string();
        };
        log_event(
            "CONNECT",
            &pseudo,
            json!({"ip": addr.to_string(), "type": "returning"}),
        );
        announce_arrival(tx, addr, &pseudo, "is back!", &enter_room, &skin, px, py);
        let exits = format_exits(world, &enter_room);
        let pos_str = saved_pos
            .map(|(x, y)| format!(" pos={},{}", x as i64, y as i64))
            .unwrap_or_default();
        return format!(
            "S: OK connected skin={} name={} room={}{}{}\n{}{}{}\nS: EVT PLAYER_STATS {} {} {} {} {}\nS: EVT GOLD {}\nS: EVT STAT_POINTS {}\n{}{}{}\n",
            skin, pseudo, enter_room, exits, pos_str,
            room_presence_roster(addr, &guard, &enter_room),
            room_mob_roster(&enter_room, &guard, world),
            room_item_roster(addr, &enter_room, &guard, world),
            p.hp, p.max_hp, p.xp_bar.current, p.xp_bar.requiered, p.level,
            p.money, p.stat_points,
            get_inv_data_event(p, world),
            get_equip_data_event(p, world),
            get_char_stats_event_with_world(p, world),
        );    }

    guard.add_player(addr, pseudo.clone());
    if guest {
        if let Some(p) = guard.players.get_mut(&addr) {
            p.guest = true;
        }
    }
    let Some(player) = guard.players.get(&addr) else {
        return "S: ERR internal_error\n".to_string();
    };
    let skin = player.skin.clone();
    let enter_room = player.current_room.clone();
    let kind = if guest { "guest" } else { "new" };
    log_event(
        "CONNECT",
        &pseudo,
        json!({"ip": addr.to_string(), "type": kind}),
    );
    let (px, py) = (player.pos_x as i64, player.pos_y as i64);
    announce_arrival(tx, addr, &pseudo, "just connected!", &enter_room, &skin, px, py);
    let exits = format_exits(world, &enter_room);
    let pos_str = format!(" pos={},{}", px, py);
    format!(
        "S: OK connected skin={} name={} room={}{}{}\n{}{}{}\nS: EVT PLAYER_STATS {} {} {} {} {}\nS: EVT GOLD {}\n",
        skin,
        pseudo,
        enter_room,
        exits,
        pos_str,
        room_presence_roster(addr, &guard, &enter_room),
        room_mob_roster(&enter_room, &guard, world),
        room_item_roster(addr, &enter_room, &guard, world),
        player.hp,
        player.max_hp,
        player.xp_bar.current,
        player.xp_bar.requiered,
        player.level,
        player.money
    )
}

/// Tells everybody that `pseudo` arrived: global chat line, then presence in the room.
#[allow(clippy::too_many_arguments)]
pub(crate) fn announce_arrival(
    tx: &Sender<GlobalEvent>,
    addr: SocketAddr,
    pseudo: &str,
    chat: &str,
    room: &RoomId,
    skin: &str,
    px: i64,
    py: i64,
) {
    to_all(
        tx,
        Some(addr),
        format!("S: EVT GLOBAL CHAT Server {} {}\n", pseudo, chat),
    );
    to_room(
        tx,
        Some(addr),
        format!("S: EVT ROOM PRESENCE ENTER {}\n", pseudo),
        room.clone(),
    );
    to_room(
        tx,
        Some(addr),
        presence_roster_line(pseudo, skin, px, py),
        room.clone(),
    );
}
