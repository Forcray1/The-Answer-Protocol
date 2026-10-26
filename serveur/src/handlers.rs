use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use rand::Rng;
use serde_json::json;
use tokio::sync::broadcast::Sender;

use domain::{xp_required_for_level, Armor, CombatState, Direction, Equipement, Group, GroupId, ItemBucket, ItemId, PlayerId, RoomId, Weapon, WeaponType};
use domain::quest::{QuestProgress, QuestStage};

use crate::commands::GameCommand;
use crate::state::{CombatInstance, CombatPhase, Player, ServerState, START_ROOM};
use crate::world::{Item as CatalogItem, Npc, WorldData};
use crate::{log_event, DbPool, GlobalEvent};

fn ci_eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn reward_quest(player: &mut Player, q: &crate::world::Quest, world: &WorldData) -> String {
    player.completed_quests.push(q.id.clone());

    let mut msg = format!("[QUEST COMPLETE] {}!", q.name);
    if let Some(ref reward_id) = q.reward_item {
        let bucket = world
            .world
            .items
            .iter()
            .find(|i| &i.id == reward_id)
            .map(|i| i.bucket())
            .unwrap_or(ItemBucket::Item);
        player.inventory.add(reward_id.clone(), bucket);
        let item_name = world
            .world
            .items
            .iter()
            .find(|i| &i.id == reward_id)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| reward_id.to_string());
        msg.push_str(&format!(" You receive: {}.", item_name));
    }
    if let Some(xp) = q.reward_exp {
        player.add_xp(xp);
        msg.push_str(&format!(" (+{} EXP)", xp));
    }

    msg
}

fn make_weapon(item: &CatalogItem) -> Weapon {
    Weapon {
        id: item.id.clone(),
        name: item.name.clone(),
        damages: item.damage.unwrap_or(0),
        category: WeaponType::Melee,
        sprite: item.sprite.clone(),
    }
}

/// Builds a domain `Armor` from a catalog item.
fn make_armor(item: &CatalogItem) -> Armor {
    Armor {
        id: item.id.clone(),
        name: item.name.clone(),
        description: item.description.clone(),
        defense: item.defense.unwrap_or(0),
        special_defense: 0,
        sprite: item.sprite.clone(),
    }
}

/// Rebuilds structured equipment from the slot -> id table (DB),
/// re-reading stats from the catalog.
fn build_equipement(world: &WorldData, equip: &HashMap<String, ItemId>) -> Equipement {
    let mut equipement = Equipement::default();
    for (slot, item_id) in equip {
        if let Some(item) = world.world.items.iter().find(|i| &i.id == item_id) {
            if slot == "weapon" {
                equipement.weapon = Some(make_weapon(item));
            } else if let Some(slot_ref) = equipement.armor_slot_mut(slot) {
                *slot_ref = Some(make_armor(item));
            }
        }
    }
    equipement
}

// Async entry point

pub async fn process_command(
    addr: SocketAddr,
    command: GameCommand,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
    pool: &DbPool,
) -> (String, bool) {
    match command {
        GameCommand::Connect { username, password } => (handle_connect(addr, username, password, state, world, tx, pool).await, false),
        GameCommand::Look           => (handle_look(addr, state, world), false),
        GameCommand::Move(dir)      => (handle_move(addr, dir, state, world, tx), false),
        GameCommand::Inventory      => (handle_inventory(addr, state, world), false),
        GameCommand::Info(c)        => (handle_info(addr, c, state, world), false),
        GameCommand::Equip(c)       => (handle_equip(addr, c, state, world), false),
        GameCommand::Unequip(c)     => (handle_unequip(addr, c, state, world), false),
        GameCommand::Equipment      => (handle_equipment(addr, state), false),
        GameCommand::Take(c)        => (handle_take(addr, c, state, world), false),
        GameCommand::Drop(c)        => (handle_drop(addr, c, state, world), false),
        GameCommand::Talk(c)        => (handle_talk(addr, c, state, world, tx), false),
        GameCommand::Interact(c)    => (handle_interact(addr, c, state, world, tx), false),
        GameCommand::Attack(c)      => (handle_attack(addr, c, state, world, tx), false),
        GameCommand::Chat { channel, message } => (handle_chat(addr, channel, message, state, tx), false),
        GameCommand::Who            => (handle_who(addr, state), false),
        GameCommand::Pos { x, y }   => (handle_pos(addr, x, y, state, tx), false),
        GameCommand::Status         => (handle_status(addr, state), false),
        GameCommand::GroupCreate    => (handle_group_create(addr, state, tx), false),
        GameCommand::GroupInvite(c) => (handle_group_invite(addr, c, state, tx), false),
        GameCommand::GroupAccept    => (handle_group_accept(addr, state, tx), false),
        GameCommand::GroupLeave     => (handle_group_leave(addr, state, tx), false),
        GameCommand::GroupInfo      => (handle_group_info(addr, state), false),
        GameCommand::Quests         => (handle_quests(addr, state, world), false),
        GameCommand::Flee           => (handle_flee(addr, state), false),
        GameCommand::Answer { npc, answer } => (handle_answer(addr, npc, answer, state, world, tx), false),
        GameCommand::Quit           => ("S: OK goodbye\n".to_string(), true),
        GameCommand::Unknown        => ("S: ERR malformed_command\n".to_string(), false),
        _                           => ("S: OK command received but not implemented yet\n".to_string(), false),
    }
}

// Save called on disconnect

pub async fn save_player_to_db(pool: &DbPool, player: &Player) {
    // Structured equipment is flattened to a slot -> id table for the DB.
    let mut equipment: HashMap<String, ItemId> = HashMap::new();
    for (slot, id, _name) in player.equipement.equipped() {
        equipment.insert(slot.to_string(), id);
    }

    let result = database::save_player(
        pool,
        &player.username,
        player.hp,
        player.xp_bar.current,
        player.level,
        player.money,
        player.current_room.as_str(),
        &player.inventory,
        &equipment,
        &player.completed_quests,
        &player.skin,
        player.pos_x as f64,
        player.pos_y as f64,
    ).await;

    if let Err(e) = result {
        eprintln!("[DB] Error saving player {}: {}", player.username, e);
    } else {
        println!("[DB] Player {} saved.", player.username);
    }
}

// Connect handler adapted for DB

fn format_exits(world: &WorldData, room: &RoomId) -> String {
    let mut exits_str = String::new();
    if let Some(loc) = world.world.locations.get(room) {
        let exits: Vec<&str> = loc.exits.keys().map(|d| d.as_str()).collect();
        if !exits.is_empty() {
            exits_str = format!(" exits={}", exits.join(","));
        }
    }
    exits_str
}

fn room_mob_roster(room: &RoomId, state: &ServerState, world: &WorldData) -> String {
    let mut out = String::new();
    if let Some(npc_ids) = state.room_npcs.get(room) {
        for npc_id in npc_ids {
            if let Some(npc) = world.world.npcs.iter().find(|n| &n.id == npc_id) {
                let sprite = npc.map_sprite();
                let (x, y) = npc.spawn_pos.map(|p| (p[0] as i64, p[1] as i64)).unwrap_or((0, 0));
                let hp = state.npc_hps.get(npc_id).copied().unwrap_or(npc.hp);
                let max_hp = npc.hp;
                let scale = npc.scale.unwrap_or(1.0);
                out.push_str(&format!(
                    "S: EVT ROOM {} MOB_SPAWN {} {} {} {} {} {} {} {} {}\n",
                    room, npc_id, sprite, x, y, hp, max_hp, scale, npc.name.replace(" ", "_"), npc.role
                ));
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
async fn handle_connect(
    addr: SocketAddr,
    pseudo: String,
    password: String,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
    pool: &DbPool,
) -> String {
    if state.players.contains_key(&addr) {
        return "S: ERR you_are_already_connected\n".to_string();
    }

    if password.is_empty() {
        return "S: ERR password_required\n".to_string();
    }

    // Prevent logging in twice on the same account
    if state.players.values().any(|p| p.username == pseudo) {
        return "S: ERR already_logged_in\n".to_string();
    }

    // Try to load an existing player
    match database::load_player(pool, &pseudo).await {
        Ok(Some(data)) => {
            // Existing account → verify password before restoring
            match database::verify_player(pool, &pseudo, &password).await {
                Ok(true) => {} // Password correct, continue
                Ok(false) => {
                    log_event("AUTH_FAIL", &pseudo, json!({"ip": addr.to_string()}));
                    return "S: ERR bad_credentials\n".to_string();
                }
                Err(e) => {
                    eprintln!("[DB] Verification error: {}", e);
                    return "S: ERR internal_error\n".to_string();
                }
            }

            let saved_pos = data.position;

            // Player found → restore state into the enriched model
            let mut player = Player::new(pseudo.clone(), data.current_room);
            player.hp = data.hp;
            player.level = data.level.max(1);
            player.xp_bar.current = data.exp;
            player.xp_bar.requiered = xp_required_for_level(player.level);
            player.money = data.money;
            player.inventory = data.inventory;
            player.completed_quests = data.completed_quests;
            player.equipement = build_equipement(world, &data.equipment);
            player.skin = data.skin;
            if let Some((x, y)) = saved_pos {
                player.pos_x = x;
                player.pos_y = y;
            }

            // Capture the skin before `player` is moved into state:
            // the client uses it to display the correct avatar.
            let skin = player.skin.clone();
            let enter_room = player.current_room.clone();
            let (px, py) = (player.pos_x as i64, player.pos_y as i64);
            state.players.insert(addr, player);
            log_event("CONNECT", &pseudo, json!({"ip": addr.to_string(), "type": "returning"}));
            let _ = tx.send(GlobalEvent {
                sender_addr: Some(addr),
                message: format!("S: EVT GLOBAL CHAT Server {} is back!\n", pseudo),
                target_room: None,
                target_group: None,
            target_player: None,
            });
            let _ = tx.send(GlobalEvent {
                sender_addr: Some(addr),
                message: format!("S: EVT ROOM {} PRESENCE ENTER {} {} {} {}\n", enter_room, pseudo, skin, px, py),
                target_room: Some(enter_room.clone()),
                target_group: None,
            target_player: None,
            });
            let exits = format_exits(world, &enter_room);
            let pos_str = saved_pos
                .map(|(x, y)| format!(" pos={},{}", x as i64, y as i64))
                .unwrap_or_default();
            let p = state.players.get(&addr).unwrap();
            format!(
                "S: OK connected skin={} name={} room={}{}{}\n{}{}\nS: EVT PLAYER_STATS {} {} {} {} {}\n",
                skin, pseudo, enter_room, exits, pos_str,
                room_presence_roster(addr, state, &enter_room),
                room_mob_roster(&enter_room, state, world),
                p.hp, p.max_hp, p.xp_bar.current, p.xp_bar.requiered, p.level
            )
        }
        Ok(None) => {
            // New player → create account with the provided password
            if let Err(e) = database::register_player(pool, &pseudo, &password).await {
                eprintln!("[DB] Account creation error: {}", e);
                return "S: ERR internal_error\n".to_string();
            }
            state.add_player(addr, pseudo.clone());
            let skin = state
                .players
                .get(&addr)
                .map(|p| p.skin.clone())
                .unwrap_or_else(|| "default".to_string());
            let enter_room = state
                .players
                .get(&addr)
                .map(|p| p.current_room.clone())
                .unwrap_or_else(|| RoomId::from(START_ROOM));
            log_event("CONNECT", &pseudo, json!({"ip": addr.to_string(), "type": "new"}));
            let _ = tx.send(GlobalEvent {
                sender_addr: Some(addr),
                message: format!("S: EVT GLOBAL CHAT Server {} just connected!\n", pseudo),
                target_room: None,
                target_group: None,
            target_player: None,
            });
            let _ = tx.send(GlobalEvent {
                sender_addr: Some(addr),
                message: format!("S: EVT ROOM {} PRESENCE ENTER {} {} 0 0\n", enter_room, pseudo, skin),
                target_room: Some(enter_room.clone()),
                target_group: None,
            target_player: None,
            });
            let exits = format_exits(world, &enter_room);
            let player = state.players.get(&addr).unwrap();
            format!(
                "S: OK connected skin={} name={} room={}{}\n{}{}\nS: EVT PLAYER_STATS {} {} {} {} {}\n",
                skin, pseudo, enter_room, exits,
                room_presence_roster(addr, state, &enter_room),
                room_mob_roster(&enter_room, state, world),
                player.hp, player.max_hp, player.xp_bar.current, player.xp_bar.requiered, player.level
            )
        }
        Err(e) => {
            eprintln!("[DB] Player load error: {}", e);
            "S: ERR internal_error\n".to_string()
        }
    }
}

fn handle_look(addr: SocketAddr, state: &ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get(&addr) {
        if let Some(loc) = world.world.locations.get(&player.current_room) {
            let mut objets_text = String::from("No items on the ground.");
            if let Some(items_au_sol) = state.room_items.get(&player.current_room) {
                let noms_objets: Vec<String> = items_au_sol.iter()
                    .filter(|ri| {
                        if let Some(static_item) = world.world.items.iter().find(|i| i.id == ri.item_id) {
                            match static_item.r#type {
                                crate::world::ItemType::Standard => true,
                                crate::world::ItemType::Quest => !ri.collected_by.contains(&player.username)
                            }
                        } else { false }
                    })
                    .map(|ri| {
                        world.world.items.iter()
                            .find(|i| i.id == ri.item_id)
                            .map(|i| format!("\"{}\"", i.name))
                            .unwrap_or_else(|| format!("\"{}\"", ri.item_id))
                    }).collect();
                if !noms_objets.is_empty() {
                    objets_text = format!("Items on the ground: {}", noms_objets.join(", "));
                }
            }
            let mut npcs_text = String::from("Nobody else here.");
            if let Some(npcs_ici) = state.room_npcs.get(&player.current_room) {
                if !npcs_ici.is_empty() {
                    let noms_npcs: Vec<String> = npcs_ici.iter().map(|id| {
                        world.world.npcs.iter().find(|n| &n.id == id)
                            .map(|n| format!("\"{}\"", n.name))
                            .unwrap_or_else(|| format!("\"{}\"", id))
                    }).collect();
                    npcs_text = format!("Present: {}", noms_npcs.join(", "));
                }
            }
            format!("S: OK [{}] - {} | {} | {}\n", loc.name, loc.description, objets_text, npcs_text)
        } else { "S: ERR room_not_found\n".to_string() }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_pos(addr: SocketAddr, x: f32, y: f32, state: &mut ServerState, tx: &Sender<GlobalEvent>) -> String {
    if let Some(player) = state.players.get_mut(&addr) {
        player.pos_x = x;
        player.pos_y = y;
        let room = player.current_room.clone();
        let name = player.username.clone();
        let _ = tx.send(GlobalEvent {
            sender_addr: Some(addr),
            message: format!("S: EVT ROOM {} POS {} {} {}\n", room, name, x as i64, y as i64),
            target_room: Some(room),
            target_group: None,
            target_player: None,
        });
    }
    String::new()
}

fn room_presence_roster(addr: SocketAddr, state: &ServerState, room: &RoomId) -> String {
    let mut out = String::new();
    for (other_addr, p) in &state.players {
        if *other_addr != addr && &p.current_room == room {
            out.push_str(&format!(
                "S: EVT ROOM {} PRESENCE ROSTER {} {} {} {}\n",
                room, p.username, p.skin, p.pos_x as i64, p.pos_y as i64
            ));
        }
    }
    out
}

fn handle_move(addr: SocketAddr, dir: String, state: &mut ServerState, world: &WorldData, tx: &Sender<GlobalEvent>) -> String {
    let (old_room, next_room, username, skin, px, py) = {
        let player = match state.players.get_mut(&addr) {
            Some(p) => p,
            None => return "S: ERR utilize_connect_first\n".to_string(),
        };
        if player.last_move.map_or(false, |last| last.elapsed() < Duration::from_millis(500)) {
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
        let old_room = player.current_room.clone();
        let username = player.username.clone();
        let skin = player.skin.clone();
        let (px, py) = (player.pos_x, player.pos_y);
        player.current_room = next_room.clone();
        player.last_move = Some(Instant::now());
        (old_room, next_room, username, skin, px, py)
    };

    let _ = tx.send(GlobalEvent {
        sender_addr: Some(addr),
        message: format!("S: EVT ROOM {} PRESENCE LEAVE {}\n", old_room, username),
        target_room: Some(old_room.clone()),
        target_group: None,
            target_player: None,
    });
    let _ = tx.send(GlobalEvent {
        sender_addr: Some(addr),
        message: format!("S: EVT ROOM {} PRESENCE ENTER {} {} {} {}\n", next_room, username, skin, px as i64, py as i64),
        target_room: Some(next_room.clone()),
        target_group: None,
            target_player: None,
    });
    let exits = format_exits(world, &next_room);
    format!("S: OK room-loc.{}{}\n{}{}", next_room, exits, room_presence_roster(addr, state, &next_room), room_mob_roster(&next_room, state, world))
}

fn get_inv_data_event(player: &crate::state::Player, world: &WorldData) -> String {
    if player.inventory.is_empty() {
        return "S: EVT INV_DATA empty\n".to_string();
    }
    let mut data_objets = Vec::new();
    for (id, count) in player.inventory.iter() {
        let item_opt = world.world.items.iter().find(|i| &i.id == id);
        let name   = item_opt.map(|i| i.name.clone()).unwrap_or_else(|| id.to_string());
        let slot   = item_opt.and_then(|i| i.slot.clone()).unwrap_or_else(|| "none".to_string());
        let damage = item_opt.and_then(|i| i.damage).unwrap_or(0);
        let sprite = item_opt.and_then(|i| i.sprite.clone()).unwrap_or_else(|| "none".to_string());
        // 7th field: 1 if this item is the currently equipped medaillon
        let equipped = if player.equipped_medaillon_id.as_ref() == Some(id) { 1u8 } else { 0u8 };
        data_objets.push(format!("{}:{}:{}:{}:{}:{}:{}", id, count, slot, name, damage, sprite, equipped));
    }
    format!("S: EVT INV_DATA {}\n", data_objets.join("|"))
}

fn get_equip_data_event(player: &crate::state::Player) -> String {
    let head   = player.equipement.helmet.as_ref().map(|a| format!("{}:{}:{}:{}", a.id, a.name, a.defense, a.sprite.as_deref().unwrap_or("none"))).unwrap_or_else(|| "none".to_string());
    let chest  = player.equipement.chestplate.as_ref().map(|a| format!("{}:{}:{}:{}", a.id, a.name, a.defense, a.sprite.as_deref().unwrap_or("none"))).unwrap_or_else(|| "none".to_string());
    let legs   = player.equipement.legging.as_ref().map(|a| format!("{}:{}:{}:{}", a.id, a.name, a.defense, a.sprite.as_deref().unwrap_or("none"))).unwrap_or_else(|| "none".to_string());
    let weapon = player.equipement.weapon.as_ref().map(|w| format!("{}:{}:{}:{}", w.id, w.name, w.damages, w.sprite.as_deref().unwrap_or("none"))).unwrap_or_else(|| "none".to_string());
    // Medaillon slot: the item stays in inventory, we just send its id + name for the slot display
    let medaillon = if let Some(mid) = &player.equipped_medaillon_id {
        format!("{}:equipped:0:QuestUI", mid)
    } else {
        "none".to_string()
    };
    format!("S: EVT EQUIP_DATA head:{}|chest:{}|legs:{}|weapon:{}|medaillon:{}\n", head, chest, legs, weapon, medaillon)
}

fn get_char_stats_event(player: &crate::state::Player) -> String {
    let weapon_dmg = player.equipement.weapon_damage();
    let total_dmg = player.stats.damage + weapon_dmg;
    let total_spe_dmg = player.stats.special_damage;
    let total_def = player.stats.defense + player.equipement.total_defense();
    let total_spe_def: i32 = player.stats.special_defense
        + [&player.equipement.helmet, &player.equipement.chestplate, &player.equipement.legging, &player.equipement.boot]
            .iter()
            .filter_map(|s| s.as_ref())
            .map(|a| a.special_defense)
            .sum::<i32>();
    format!("S: EVT CHAR_STATS {} {} {} {} {}\n", total_dmg, total_spe_dmg, total_def, total_spe_def, player.max_hp)
}

fn handle_inventory(addr: SocketAddr, state: &ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get(&addr) {
        if player.inventory.is_empty() {
            format!("S: OK Your inventory is empty.\n{}{}{}", get_inv_data_event(player, world), get_equip_data_event(player), get_char_stats_event(player))
        } else {
            let mut noms_objets = Vec::new();
            for (id, count) in player.inventory.iter() {
                let item_opt = world.world.items.iter().find(|i| &i.id == id);
                let name = item_opt.map(|i| i.name.clone()).unwrap_or_else(|| id.to_string());
                if *count > 1 {
                    noms_objets.push(format!("{} (x{})", name, count));
                } else {
                    noms_objets.push(name);
                }
            }
            format!("S: OK Inventory: [{}]\n{}{}{}", noms_objets.join(", "), get_inv_data_event(player, world), get_equip_data_event(player), get_char_stats_event(player))
        }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_info(addr: SocketAddr, cible: String, state: &ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get(&addr) {
        if let Some(item) = world.world.items.iter().find(|i| ci_eq(i.id.as_str(), &cible) || ci_eq(&i.name, &cible)) {
            if player.inventory.contains(&item.id) {
                let mut stats = String::new();
                if let Some(dmg) = item.damage { stats.push_str(&format!("\n Damage: +{}", dmg)); }
                if let Some(def) = item.defense { stats.push_str(&format!("\n Armor: +{}", def)); }
                format!("S: OK --- {} ---\n {}{}\n", item.name, item.description, stats)
            } else { "S: ERR You don't have this item.\n".to_string() }
        } else { "S: ERR Unknown item.\n".to_string() }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_equip(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get_mut(&addr) {
        if let Some(item) = world.world.items.iter().find(|i| ci_eq(i.id.as_str(), &cible) || ci_eq(&i.name, &cible)) {
            if !player.inventory.contains(&item.id) {
                return "S: ERR You don't have this item.\n".to_string();
            }
            let slot = match &item.slot {
                Some(s) => s.clone(),
                None => return "S: ERR This item cannot be equipped.\n".to_string(),
            };

            if slot == "medaillon" {
                // Medaillon stays in inventory — just flag it as equipped.
                // If another medaillon is already equipped, unequip it first.
                let old_id = player.equipped_medaillon_id.replace(item.id.clone());
                let msg = if old_id.is_some() {
                    format!("S: OK Swapped medaillon for {}.\n", item.name)
                } else {
                    format!("S: OK You equipped {} in slot [medaillon].\n", item.name)
                };
                return format!("{}{}{}{}", msg, get_inv_data_event(player, world), get_equip_data_event(player), get_char_stats_event(player));
            }

            if slot == "weapon" {
                let old = player.equipement.weapon.take();
                player.equipement.weapon = Some(make_weapon(item));
                if let Some(old) = old { player.inventory.add(old.id, ItemBucket::Weapon); }
                player.inventory.remove_one(&item.id);
                format!("S: OK You equipped {} in slot [{}].\n{}{}{}", item.name, slot, get_inv_data_event(player, world), get_equip_data_event(player), get_char_stats_event(player))
            } else if let Some(slot_ref) = player.equipement.armor_slot_mut(&slot) {
                let old = slot_ref.take();
                *slot_ref = Some(make_armor(item));
                if let Some(old) = old { player.inventory.add(old.id, ItemBucket::Item); }
                player.inventory.remove_one(&item.id);
                format!("S: OK You equipped {} in slot [{}].\n{}{}{}", item.name, slot, get_inv_data_event(player, world), get_equip_data_event(player), get_char_stats_event(player))
            } else {
                "S: ERR This item cannot be equipped.\n".to_string()
            }
        } else { "S: ERR Unknown item.\n".to_string() }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_unequip(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get_mut(&addr) {
        // Medaillon unequip: just clear the flag, item stays in inventory.
        if let Some(mid) = &player.equipped_medaillon_id.clone() {
            if ci_eq(mid.as_str(), &cible) {
                player.equipped_medaillon_id = None;
                return format!("S: OK You unequipped the medaillon.\n{}{}{}", get_inv_data_event(player, world), get_equip_data_event(player), get_char_stats_event(player));
            }
        }

        let found = player.equipement.equipped().into_iter()
            .find(|(_, id, name)| ci_eq(id.as_str(), &cible) || ci_eq(name, &cible));
        if let Some((slot, id, name)) = found {
            if slot == "weapon" {
                player.equipement.weapon = None;
                player.inventory.add(id, ItemBucket::Weapon);
            } else if let Some(slot_ref) = player.equipement.armor_slot_mut(slot) {
                *slot_ref = None;
                player.inventory.add(id, ItemBucket::Item);
            }
            format!("S: OK You unequipped {}.\n{}{}{}", name, get_inv_data_event(player, world), get_equip_data_event(player), get_char_stats_event(player))
        } else { "S: ERR You are not wearing this item.\n".to_string() }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_equipment(addr: SocketAddr, state: &ServerState) -> String {
    if let Some(player) = state.players.get(&addr) {
        let aucun = || "None".to_string();
        let head   = player.equipement.helmet.as_ref().map(|a| a.name.clone()).unwrap_or_else(aucun);
        let chest  = player.equipement.chestplate.as_ref().map(|a| a.name.clone()).unwrap_or_else(aucun);
        let legs   = player.equipement.legging.as_ref().map(|a| a.name.clone()).unwrap_or_else(aucun);
        let weapon = player.equipement.weapon.as_ref().map(|w| w.name.clone()).unwrap_or_else(aucun);
        format!("S: OK --- EQUIPMENT ---\n  Head: {}\n  Chest: {}\n  Legs: {}\n  Weapon: {}\n{}{}", head, chest, legs, weapon, get_equip_data_event(player), get_char_stats_event(player))
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_take(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get_mut(&addr) {
        let current_room_id = player.current_room.clone();
        if let Some(item) = world.world.items.iter().find(|i| ci_eq(i.id.as_str(), &cible) || ci_eq(&i.name, &cible)) {
            if let Some(items_in_room) = state.room_items.get_mut(&current_room_id) {
                if let Some(index) = items_in_room.iter().position(|ri| ri.item_id == item.id) {
                    let mut deja_ramasse = false;
                    match item.r#type {
                        crate::world::ItemType::Standard => { items_in_room.remove(index); }
                        crate::world::ItemType::Quest => {
                            let runtime_item = &mut items_in_room[index];
                            if runtime_item.collected_by.contains(&player.username) { deja_ramasse = true; }
                            else { runtime_item.collected_by.insert(player.username.clone()); }
                        }
                    }
                    if deja_ramasse {
                        "S: ERR You have already taken this quest item.\n".to_string()
                    } else {
                        player.inventory.add(item.id.clone(), item.bucket());
                        log_event("TAKE", &player.username, json!({"item_id": item.id}));
                        let mut quest_msg = String::new();
                        for q in &world.world.quests {
                            if let crate::world::QuestObjective::FetchItem { item: target_id } = &q.objective {
                                if target_id == &item.id && !player.completed_quests.contains(&q.id) {
                                    player.completed_quests.push(q.id.clone());
                                    if let Some(xp) = q.reward_exp {
                                        player.add_xp(xp);
                                        quest_msg = format!(" [QUEST COMPLETE] {}! (+{} EXP)", q.name, xp);
                                    }
                                }
                            }
                        }
                        format!("S: OK You picked up: {}{}\n", item.name, quest_msg)
                    }
                } else { "S: ERR This item is not here.\n".to_string() }
            } else { "S: ERR room_error\n".to_string() }
        } else { "S: ERR Unknown item.\n".to_string() }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_drop(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get_mut(&addr) {
        if let Some(item) = world.world.items.iter().find(|i| ci_eq(i.id.as_str(), &cible) || ci_eq(&i.name, &cible)) {
            if player.inventory.contains(&item.id) {
                if item.r#type == crate::world::ItemType::Quest {
                    "S: ERR You cannot get rid of a quest item!\n".to_string()
                } else {
                    player.inventory.remove_one(&item.id);
                    let current_room_id = player.current_room.clone();
                    state.room_items.entry(current_room_id).or_default().push(crate::state::RuntimeItem {
                        item_id: item.id.clone(),
                        source: crate::state::ItemSource::PlayerDrop,
                        collected_by: std::collections::HashSet::new(),
                    });
                    log_event("DROP", &player.username, json!({"item_id": item.id}));
                    format!("S: OK You dropped on the ground: {}\n", item.name)
                }
            } else { format!("S: ERR You don't have the item \"{}\".\n", item.name) }
        } else { "S: ERR Unknown item.\n".to_string() }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

// ── Quest system helpers ────────────────────────────────────────────────

/// Finds a player's progress for a given quest.
#[allow(dead_code)]
fn quest_progress_for<'a>(
    player: &'a crate::state::Player,
    quest_id: &domain::QuestId,
) -> Option<&'a QuestProgress> {
    player.quest_progress.iter().find(|p| &p.quest_id == quest_id)
}

fn quest_progress_for_mut<'a>(
    player: &'a mut crate::state::Player,
    quest_id: &domain::QuestId,
) -> Option<&'a mut QuestProgress> {
    player.quest_progress.iter_mut().find(|p| &p.quest_id == quest_id)
}

/// Advances a quest stage and sends EVT QUEST_UPDATE to the player.
fn advance_quest_stage(
    addr: SocketAddr,
    quest_id: &domain::QuestId,
    player: &mut crate::state::Player,
    tx: &Sender<GlobalEvent>,
) {
    if let Some(prog) = quest_progress_for_mut(player, quest_id) {
        prog.stage = prog.stage.next();
        prog.counter = 0;
        let stage_wire = prog.stage.to_wire();
        let _ = tx.send(GlobalEvent {
            sender_addr: None,
            message: format!("S: EVT QUEST_UPDATE {} {} 0\n", quest_id, stage_wire),
            target_room: None,
            target_group: None,
            target_player: Some(addr),
        });
    }
}

/// Starts a quest (stage NotStarted → Stage 1) if not already tracked.
fn start_quest(
    addr: SocketAddr,
    quest: &crate::world::Quest,
    player: &mut crate::state::Player,
    tx: &Sender<GlobalEvent>,
) {
    if player.completed_quests.contains(&quest.id) {
        return;
    }
    if player.quest_progress.iter().any(|p| p.quest_id == quest.id) {
        return;
    }
    let mut prog = QuestProgress::new(quest.id.clone());
    prog.stage = QuestStage::Stage(1);
    player.quest_progress.push(prog);
    let _ = tx.send(GlobalEvent {
        sender_addr: None,
        message: format!("S: EVT QUEST_UPDATE {} 1 0\n", quest.id),
        target_room: None,
        target_group: None,
        target_player: Some(addr),
    });
}

/// Selects the most relevant conditional dialogue block for an NPC
/// based on the current stage of the first active quest linked to this NPC.
fn pick_quest_dialogue<'a>(
    npc: &'a crate::world::Npc,
    player: &crate::state::Player,
    world: &WorldData,
) -> Option<&'a crate::world::QuestDialogue> {
    // For each quest linked to this NPC (giver_id OR objective target), find the right block.
    for q in &world.world.quests {
        let is_giver = q.giver_id.as_ref() == Some(&npc.id);
        let is_target = is_npc_quest_target(npc, q);
        if !is_giver && !is_target { continue; }

        if player.completed_quests.contains(&q.id) {
            // Look for a stage = 255 (Completed) block
            let maybe = npc.quest_dialogues.iter().find(|qd| qd.quest_id == q.id && qd.stage == 255);
            if maybe.is_some() { return maybe; }
            continue;
        }

        let current_stage_num = player.quest_progress.iter()
            .find(|p| p.quest_id == q.id)
            .map(|p| match &p.stage {
                QuestStage::NotStarted => 0,
                QuestStage::Stage(n)   => *n,
                QuestStage::Completed  => 255,
            })
            .unwrap_or(0); // 0 = not yet started

        let maybe = npc.quest_dialogues.iter().find(|qd| qd.quest_id == q.id && qd.stage == current_stage_num);
        if maybe.is_some() { return maybe; }
    }
    None
}

fn is_npc_quest_target(npc: &crate::world::Npc, q: &crate::world::Quest) -> bool {
    use crate::world::QuestObjective;
    match &q.objective {
        QuestObjective::TalkToNpc { npc: n }         => n == &npc.id,
        QuestObjective::DeliverItem { to, .. }        => to == &npc.id,
        QuestObjective::DeliverItems { to, .. }       => to == &npc.id,
        _ => false,
    }
}

fn handle_talk(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData, tx: &Sender<GlobalEvent>) -> String {
    // ── 1. Find the NPC ─────────────────────────────────────────────────
    let current_room_id;
    let npc_trouve: Option<crate::world::Npc>;
    {
        let player = match state.players.get(&addr) {
            Some(p) => p,
            None => return "S: ERR utilize_connect_first\n".to_string(),
        };
        current_room_id = player.current_room.clone();
        npc_trouve = state.room_npcs.get(&current_room_id).and_then(|ids| {
            ids.iter().find_map(|npc_id| {
                world.world.npcs.iter().find(|n| {
                    &n.id == npc_id && (ci_eq(n.id.as_str(), &cible) || ci_eq(&n.name, &cible))
                }).cloned()
            })
        });
    }

    let npc = match npc_trouve {
        Some(n) => n,
        None => return "S: ERR There is nobody by that name here.\n".to_string(),
    };

    // ── 2. Quest resolution (legacy: DeliverItem / TalkToNpc) ──────────
    let mut quest_msgs: Vec<String> = Vec::new();
    {
        let player = state.players.get_mut(&addr).unwrap();
        for q in &world.world.quests {
            if player.completed_quests.contains(&q.id) { continue; }

            // Multi-stage quests: check TalkToNpc at the current stage
            if !q.stages.is_empty() {
                let current_stage = player.quest_progress.iter()
                    .find(|p| p.quest_id == q.id)
                    .map(|p| p.stage.clone())
                    .unwrap_or(QuestStage::NotStarted);

                if let Some(stage_num) = current_stage.stage_num() {
                    if let Some(stage_def) = q.stages.iter().find(|s| s.stage == stage_num) {
                        match &stage_def.objective {
                            Some(domain::quest::StageObjective::TalkToNpc { npc: target }) if target == &npc.id => {
                                // Last stage → complete the quest
                                let is_last = q.stages.iter().all(|s| s.stage <= stage_num);
                                if is_last {
                                    let qid = q.id.clone();
                                    if let Some(prog) = quest_progress_for_mut(player, &qid) {
                                        prog.stage = QuestStage::Completed;
                                    }
                                    player.completed_quests.push(q.id.clone());
                                    player.quest_progress.retain(|p| p.quest_id != q.id);
                                    quest_msgs.push(reward_quest(player, q, world));
                                    let _ = tx.send(GlobalEvent {
                                        sender_addr: None,
                                        message: format!("S: EVT QUEST_UPDATE {} completed 0\n", q.id),
                                        target_room: None,
                                        target_group: None,
                                        target_player: Some(addr),
                                    });
                                } else {
                                    let qid = q.id.clone();
                                    advance_quest_stage(addr, &qid, player, tx);
                                }
                            }
                            Some(domain::quest::StageObjective::FetchItem { item })
                                if q.giver_id.as_ref() == Some(&npc.id) && player.inventory.contains(item) =>
                            {
                                // Remove item from inventory and advance
                                let item_id = item.clone();
                                player.inventory.remove_one(&item_id);
                                let is_last = q.stages.iter().all(|s| s.stage <= stage_num);
                                if is_last {
                                    let qid = q.id.clone();
                                    if let Some(prog) = quest_progress_for_mut(player, &qid) {
                                        prog.stage = QuestStage::Completed;
                                    }
                                    player.completed_quests.push(q.id.clone());
                                    player.quest_progress.retain(|p| p.quest_id != q.id);
                                    quest_msgs.push(reward_quest(player, q, world));
                                    let _ = tx.send(GlobalEvent {
                                        sender_addr: None,
                                        message: format!("S: EVT QUEST_UPDATE {} completed 0\n", q.id),
                                        target_room: None,
                                        target_group: None,
                                        target_player: Some(addr),
                                    });
                                } else {
                                    let qid = q.id.clone();
                                    advance_quest_stage(addr, &qid, player, tx);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                continue;
            }

            // Legacy quests (single objective)
            match &q.objective {
                crate::world::QuestObjective::DeliverItem { item, to } => {
                    if &npc.id == to && player.inventory.contains(item) {
                        player.inventory.remove_one(item);
                        quest_msgs.push(reward_quest(player, q, world));
                    }
                }
                crate::world::QuestObjective::DeliverItems { items, to } => {
                    if &npc.id != to { continue; }
                    let mut required_counts: HashMap<domain::ItemId, u32> = HashMap::new();
                    for id in items { *required_counts.entry(id.clone()).or_insert(0) += 1; }
                    let can_complete = required_counts.iter().all(|(id, needed)| player.inventory.count(id) >= *needed);
                    if can_complete {
                        for (id, needed) in required_counts {
                            for _ in 0..needed { player.inventory.remove_one(&id); }
                        }
                        quest_msgs.push(reward_quest(player, q, world));
                    }
                }
                crate::world::QuestObjective::TalkToNpc { npc: target_npc } => {
                    if &npc.id == target_npc {
                        quest_msgs.push(reward_quest(player, q, world));
                    }
                }
                crate::world::QuestObjective::FetchItem { .. }
                | crate::world::QuestObjective::DefeatNpc { .. } => {
                    if let Some(legacy_target) = &q.target_id {
                        if q.giver_id.as_ref() == Some(&npc.id) && player.inventory.contains(legacy_target) {
                            player.inventory.remove_one(legacy_target);
                            quest_msgs.push(reward_quest(player, q, world));
                        }
                    }
                }
            }
        }
    }

    if !quest_msgs.is_empty() {
        return format!("S: OK {} {}\n", npc.name, quest_msgs.join(" "));
    }

    // ── 3. Start a quest if this NPC is the giver and the player
    //       has not started it yet ─────────────────────────────────────────
    {
        let player = state.players.get_mut(&addr).unwrap();
        for q in &world.world.quests {
            if q.giver_id.as_ref() == Some(&npc.id)
                && !player.completed_quests.contains(&q.id)
                && !player.quest_progress.iter().any(|p| p.quest_id == q.id)
            {
                start_quest(addr, q, player, tx);
            }
        }
    }

    // ── 4. Pick the right dialogue (conditional or default) ─────────────
    let player = state.players.get(&addr).unwrap();
    let npc_key = npc.name.replace(' ', "_");

    if let Some(qd) = pick_quest_dialogue(&npc, player, world) {
        if qd.choices.is_empty() {
            // Dialogue simple
            let pages = qd.lines.join("|");
            let _ = tx.send(GlobalEvent {
                sender_addr: None,
                message: format!("S: EVT DIALOGUE {} {}\n", npc_key, pages),
                target_room: None,
                target_group: None,
                target_player: Some(addr),
            });
        } else {
            // Choice dialogue: format `S: EVT DIALOGUE_CHOICE <npc_key> <pages>||CHOICES||<text>:<answer>|…`
            let pages = qd.lines.join("|");
            let choices_str = qd.choices.iter()
                .map(|c| format!("{}:{}", c.text.replace(' ', "_"), c.answer))
                .collect::<Vec<_>>()
                .join("|");
            let _ = tx.send(GlobalEvent {
                sender_addr: None,
                message: format!("S: EVT DIALOGUE_CHOICE {} {}||CHOICES||{}\n", npc_key, pages, choices_str),
                target_room: None,
                target_group: None,
                target_player: Some(addr),
            });
        }
    } else {
        let dialog_pages = npc.dialogue.join("|");
        let _ = tx.send(GlobalEvent {
            sender_addr: None,
            message: format!("S: EVT DIALOGUE {} {}\n", npc_key, dialog_pages),
            target_room: None,
            target_group: None,
            target_player: Some(addr),
        });
    }

    "S: OK interaction_started\n".to_string()
}

/// Handles the player's answer to a multiple-choice dialogue.
fn handle_answer(
    addr: SocketAddr,
    npc_id: String,
    answer: String,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    let Some(player) = state.players.get(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };

    // Find the quest expecting an answer at this stage
    let matching_quest: Option<(domain::QuestId, bool, String, Option<crate::world::OnWrongAnswer>)> = world.world.quests.iter().find_map(|q| {
        if player.completed_quests.contains(&q.id) { return None; }

        let current_stage = player.quest_progress.iter()
            .find(|p| p.quest_id == q.id)?
            .stage.clone();
        let stage_num = current_stage.stage_num()?;
        let stage_def = q.stages.iter().find(|s| s.stage == stage_num)?;

        if let Some(domain::quest::StageObjective::AnswerDialogue { npc, expected_answer }) = &stage_def.objective {
            if ci_eq(npc.as_str(), &npc_id) {
                let is_last = q.stages.iter().all(|s| s.stage <= stage_num);
                return Some((q.id.clone(), is_last, expected_answer.clone(), stage_def.on_wrong_answer.clone()));
            }
        }
        None
    });

    match matching_quest {
        None => "S: ERR no_pending_dialogue_answer\n".to_string(),
        Some((quest_id, is_last, expected, wrong_answer_cfg)) => {
            if !ci_eq(&answer, &expected) {
                // Wrong answer: apply on_wrong_answer behaviour if defined.
                let mut out = String::new();
                if let Some(cfg) = wrong_answer_cfg {
                    if cfg.reset {
                        // Remove progress → player can restart.
                        state.players.get_mut(&addr).unwrap()
                            .quest_progress.retain(|p| p.quest_id != quest_id);
                        let _ = tx.send(GlobalEvent {
                            sender_addr: None,
                            message: format!("S: EVT QUEST_RESET {}\n", quest_id),
                            target_room: None,
                            target_group: None,
                            target_player: Some(addr),
                        });
                    }
                    if !cfg.dialogue.is_empty() {
                        let pages = cfg.dialogue.join("|");
                        let _ = tx.send(GlobalEvent {
                            sender_addr: None,
                            message: format!("S: EVT DIALOGUE {} {}\n", npc_id, pages),
                            target_room: None,
                            target_group: None,
                            target_player: Some(addr),
                        });
                    }
                }
                out.push_str("S: OK wrong_answer\n");
                return out;
            }

            // Correct answer
            let player = state.players.get_mut(&addr).unwrap();
            if is_last {
                let q = world.world.quests.iter().find(|q| q.id == quest_id).unwrap().clone();
                if let Some(prog) = quest_progress_for_mut(player, &quest_id) {
                    prog.stage = QuestStage::Completed;
                }
                player.completed_quests.push(quest_id.clone());
                player.quest_progress.retain(|p| p.quest_id != quest_id);
                let reward_msg = reward_quest(player, &q, world);
                let _ = tx.send(GlobalEvent {
                    sender_addr: None,
                    message: format!("S: EVT QUEST_UPDATE {} completed 0\n", quest_id),
                    target_room: None,
                    target_group: None,
                    target_player: Some(addr),
                });
                format!("S: OK correct_answer {}\n", reward_msg)
            } else {
                advance_quest_stage(addr, &quest_id, player, tx);
                "S: OK correct_answer\n".to_string()
            }
        }
    }
}

fn get_combat_bg_for_room(room: &RoomId) -> u32 {
    let r = room.as_str().to_lowercase();
    if r.contains("cave") {
        3
    } else if r.contains("temple") {
        4
    } else if r.contains("castle") || r.contains("city") {
        5
    } else if r.contains("oasis") {
        2
    } else {
        1
    }
}

pub fn handle_flee(addr: SocketAddr, state: &mut ServerState) -> String {
    if let Some(player) = state.players.get_mut(&addr) {
        player.combat = CombatState::Idle;
    }
    state.active_combats.remove(&addr);
    "S: EVT COMBAT END flee\nS: OK You fled from combat.\n".to_string()
}

fn start_combat(addr: SocketAddr, npc: &Npc, state: &mut ServerState) -> String {
    let player = match state.players.get_mut(&addr) {
        Some(p) => p,
        None => return "S: ERR utilize_connect_first\n".to_string(),
    };

    let room = player.current_room.clone();
    player.combat = CombatState::InCombat { target: npc.id.clone() };
    state.active_combats.insert(addr, CombatInstance {
        npc_id: npc.id.clone(),
        phase: CombatPhase::WaitingForPlayerAction,
        turn_number: 1,
    });

    let current_hp = state.npc_hps.get(&npc.id).copied().unwrap_or(npc.hp);
    let bg_index = get_combat_bg_for_room(&room);
    let combat_sprite = npc.combat_sprite();

    format!(
        "S: EVT COMBAT START {} {} {} {} {} {}\n",
        npc.id,
        combat_sprite,
        current_hp,
        npc.hp,
        bg_index,
        npc.name.replace(" ", "_")
    )
}

fn handle_interact(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData, tx: &Sender<GlobalEvent>) -> String {
    let mut role = String::new();
    let mut found_npc = None;
    if let Some(player) = state.players.get(&addr) {
        let current_room_id = player.current_room.clone();
        if let Some(npcs_ici) = state.room_npcs.get(&current_room_id) {
            for npc_id in npcs_ici {
                if let Some(npc) = world.world.npcs.iter().find(|n| &n.id == npc_id) {
                    if ci_eq(npc.id.as_str(), &cible) || ci_eq(&npc.name, &cible) {
                        role = npc.role.clone();
                        found_npc = Some(npc.clone());
                        break;
                    }
                }
            }
        }
    }

    if role == "enemy" {
        if let Some(npc) = found_npc {
            let in_combat = state.players.get(&addr).map_or(false, |p| match &p.combat {
                CombatState::InCombat { target } => target == &npc.id,
                _ => false,
            });
            if in_combat {
                handle_attack(addr, cible, state, world, tx)
            } else {
                start_combat(addr, &npc, state)
            }
        } else {
            "S: ERR There is nobody by that name here.\n".to_string()
        }
    } else if !role.is_empty() {
        handle_talk(addr, cible, state, world, tx)
    } else {
        "S: ERR There is nothing to interact with here.\n".to_string()
    }
}

fn handle_attack(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData, tx: &Sender<GlobalEvent>) -> String {
    let current_room_id;
    let mut player_damage = 10;
    let mut player_armor = 0;
    let mut monster_id_found = None;
    let mut monster_name = String::new();
    let player_name;
    let mut degats_monstre = 5;
    let mut monster_defense = 0;
    let mut exp_gained = 0;
    let mut monster_drops = Vec::new();
    let mut max_hp_monster = 1;
    let mut on_cooldown = false;

    if let Some(player) = state.players.get(&addr) {
        if player.last_attack.map_or(false, |last| last.elapsed() < Duration::from_millis(1000)) { on_cooldown = true; }
    }
    if on_cooldown { return "S: ERR attack_cooldown\n".to_string(); }

    if let Some(player) = state.players.get_mut(&addr) {
        current_room_id = player.current_room.clone();
        player_name = player.username.clone();
        player.last_attack = Some(Instant::now());
        // Damage = base + equipped weapon; armor = sum of equipped pieces.
        player_damage += player.equipement.weapon_damage();
        player_armor += player.equipement.total_defense();
    } else { return "S: ERR utilize_connect_first\n".to_string(); }

    if let Some(npcs_in_room) = state.room_npcs.get(&current_room_id) {
        for npc_id in npcs_in_room {
            if let Some(npc) = world.world.npcs.iter().find(|n| &n.id == npc_id) {
                if npc.role == "enemy" && (ci_eq(npc.id.as_str(), &cible) || ci_eq(&npc.name, &cible)) {
                    monster_id_found = Some(npc.id.clone());
                    monster_name = npc.name.clone();
                    degats_monstre = npc.damage.unwrap_or(5);
                    monster_defense = npc.defense.unwrap_or(0);
                    exp_gained = npc.exp_reward.unwrap_or(0);
                    monster_drops = npc.drops.clone();
                    max_hp_monster = npc.hp;
                    break;
                }
            }
        }
    }

    if let Some(m_id) = monster_id_found {
        let mut monster_dead = false;
        let mut monster_hp_left = 0;
        let final_damage = (player_damage - monster_defense).max(1);
        if let Some(hp) = state.npc_hps.get_mut(&m_id) {
            *hp -= final_damage;
            monster_hp_left = *hp;
            if *hp <= 0 { monster_dead = true; }
        }
        if monster_dead {
            if let Some(player) = state.players.get_mut(&addr) {
                player.combat = CombatState::Idle;
            }
            state.active_combats.remove(&addr);

            if let Some(npcs_in_room) = state.room_npcs.get_mut(&current_room_id) {
                npcs_in_room.retain(|id| id != &m_id);
            }
            if let Some(npc_static) = world.world.npcs.iter().find(|n| n.id == m_id) {
                let delay = npc_static.respawn_time.unwrap_or(30);
                state.dead_npcs.push(crate::state::DeadNpc {
                    npc_id: m_id.clone(), room_id: current_room_id.clone(),
                    respawn_at: Instant::now() + Duration::from_secs(delay),
                });
            }
            let mut drop_names = Vec::new();
            let mut stat_str = String::new();
            if let Some(player) = state.players.get_mut(&addr) {
                player.add_xp(exp_gained);
                for drop in monster_drops {
                    if rand::thread_rng().gen_range(1..=100) <= drop.chance {
                        let bucket = world.world.items.iter().find(|i| i.id == drop.item_id).map(|i| i.bucket()).unwrap_or(ItemBucket::Item);
                        player.inventory.add(drop.item_id.clone(), bucket);
                        let nom = world.world.items.iter().find(|i| i.id == drop.item_id).map(|i| i.name.clone()).unwrap_or_else(|| drop.item_id.to_string());
                        drop_names.push(nom);
                    }
                }
                stat_str = format!("S: EVT PLAYER_STATS {} {} {} {} {}\n", player.hp, player.max_hp, player.xp_bar.current, player.xp_bar.requiered, player.level);

                // ── Suivi KillMob ──────────────────────────────────────────────
                let mut quests_to_advance: Vec<domain::QuestId> = Vec::new();
                let mut quests_to_complete: Vec<domain::QuestId> = Vec::new();
                for prog in &mut player.quest_progress {
                    if let QuestStage::Stage(stage_num) = prog.stage {
                        if let Some(q) = world.world.quests.iter().find(|q| q.id == prog.quest_id) {
                            if let Some(stage_def) = q.stages.iter().find(|s| s.stage == stage_num) {
                                if let Some(domain::quest::StageObjective::KillMob { npc, count }) = &stage_def.objective {
                                    if ci_eq(npc.as_str(), m_id.as_str()) {
                                        prog.counter += 1;
                                        let is_last_stage = q.stages.iter().all(|s| s.stage <= stage_num);
                                        if prog.counter >= *count {
                                            if is_last_stage {
                                                quests_to_complete.push(prog.quest_id.clone());
                                            } else {
                                                quests_to_advance.push(prog.quest_id.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                for qid in quests_to_advance {
                    advance_quest_stage(addr, &qid, player, tx);
                }
                for qid in quests_to_complete {
                    if let Some(q) = world.world.quests.iter().find(|q| q.id == qid).cloned() {
                        if let Some(prog) = quest_progress_for_mut(player, &q.id) {
                            prog.stage = QuestStage::Completed;
                        }
                        player.completed_quests.push(q.id.clone());
                        player.quest_progress.retain(|p| p.quest_id != q.id);
                        let msg = reward_quest(player, &q, world);
                        stat_str.push_str(&format!("S: OK {}\n", msg));
                        let _ = tx.send(GlobalEvent {
                            sender_addr: None,
                            message: format!("S: EVT QUEST_UPDATE {} completed 0\n", q.id),
                            target_room: None,
                            target_group: None,
                            target_player: Some(addr),
                        });
                    }
                }
            }
            let _ = tx.send(GlobalEvent {
                sender_addr: None, // Broadcast to all (mob despawn)
                message: format!("S: EVT ROOM {} MOB_DESPAWN {}\n", current_room_id, m_id),
                target_room: Some(current_room_id.clone()),
                target_group: None,
                target_player: None,
            });
            let mut msg = format!("S: EVT COMBAT END victory\nS: OK You dealt {} damage. {} collapses! (+{} EXP)\n", final_damage, monster_name, exp_gained);
            if !drop_names.is_empty() { msg.push_str(&format!("You obtain: {}\n", drop_names.join(", "))); }
            msg.push_str(&stat_str);
            msg
        } else {
            let mut stat_str = String::new();
            let mut player_dead = false;
            let mut player_hp = 0;
            let final_monster_damage = (degats_monstre - player_armor).max(1);
            if let Some(player) = state.players.get_mut(&addr) {
                player.hp -= final_monster_damage;
                player_hp = player.hp;
                if player.hp <= 0 {
                    player_dead = true;
                    player.hp = player.max_hp;
                    player.current_room = RoomId::from(START_ROOM);
                    player.combat = CombatState::Idle;
                }
                stat_str = format!("S: EVT PLAYER_STATS {} {} {} {} {}\n", player.hp, player.max_hp, player.xp_bar.current, player.xp_bar.requiered, player.level);
            }
            if player_dead {
                state.active_combats.remove(&addr);
                let _ = tx.send(GlobalEvent { sender_addr: Some(addr), message: format!("S: EVT GLOBAL CHAT Server A player was killed by {}!\n", monster_name), target_room: None, target_group: None, target_player: None });
                log_event("DEATH", &player_name, json!({"killer": monster_name}));
                format!("S: EVT COMBAT END defeat\nS: OK You deal {} damage, but {} finishes you off. You are DEAD! You wake up at the oasis.\n{}", final_damage, monster_name, stat_str)
            } else {
                format!("S: EVT COMBAT HIT {} {} {} {}\nS: OK You attack {} ({} HP left). It retaliates (-{} HP). (Your HP: {})\n{}", monster_hp_left, max_hp_monster, final_damage, final_monster_damage, monster_name, monster_hp_left, final_monster_damage, player_hp, stat_str)
            }
        }
    } else { "S: ERR You can't attack that.\n".to_string() }
}

fn handle_chat(addr: SocketAddr, channel: String, message: String, state: &mut ServerState, tx: &Sender<GlobalEvent>) -> String {
    let mut on_cooldown = false;
    let mut current_room = String::new();
    if let Some(player) = state.players.get(&addr) {
        if player.last_chat.map_or(false, |last| last.elapsed() < Duration::from_millis(2000)) { on_cooldown = true; }
        else { current_room = player.current_room.as_str().to_string(); }
    }
    if on_cooldown { return "S: ERR chat_spam_forbidden\n".to_string(); }
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
                let _ = tx.send(GlobalEvent { sender_addr: None, message: format!("S: EVT GLOBAL CHAT {} {}\n", username, message), target_room: None, target_group: None, target_player: None });
                "S: OK\n".to_string()
            }
            "ROOM" => {
                let _ = tx.send(GlobalEvent { sender_addr: None, message: format!("S: EVT ROOM {} CHAT {} {}\n", current_room, username, message), target_room: Some(RoomId::from(current_room.as_str())), target_group: None, target_player: None });
                "S: OK\n".to_string()
            }
            "GROUP" => {
                let _ = tx.send(GlobalEvent { sender_addr: None, message: format!("S: EVT GROUP CHAT {} {}\n", username, message), target_room: None, target_group: group_id_opt, target_player: None });
                "S: OK\n".to_string()
            }
            _ => "S: ERR unknown_channel\n".to_string(),
        }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_who(addr: SocketAddr, state: &ServerState) -> String {
    let total_serveur = state.players.len();
    if let Some(current_player) = state.players.get(&addr) {
        let players_in_room: Vec<String> = state.players.values()
            .filter(|p| p.current_room == current_player.current_room)
            .map(|p| format!("\"{}\"", p.username))
            .collect();
        format!("S: OK {{ \"room\": [{}], \"server\": {} }}\n", players_in_room.join(", "), total_serveur)
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_status(addr: SocketAddr, state: &ServerState) -> String {
    if let Some(player) = state.players.get(&addr) {
        format!(
            "S: OK HP: {}/{} | Lv: {} | XP: {}/{} | Gold: {} | Location: {}\n",
            player.hp, player.max_hp, player.level,
            player.xp_bar.current, player.xp_bar.requiered,
            player.money, player.current_room
        )
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

// ── GROUP ──────────────────────────────────────────────────────────────────

fn handle_group_create(addr: SocketAddr, state: &mut ServerState, tx: &Sender<GlobalEvent>) -> String {
    if let Some(player) = state.players.get(&addr) {
        if player.group.is_some() {
            return "S: ERR you_already_have_a_group\n".to_string();
        }
        let username = player.username.clone();
        let player_id = player.id.clone();

        state.group_counter += 1;
        let gid = GroupId(format!("group_{}", state.group_counter));
        let group = Group {
            id: gid.clone(),
            members: vec![player_id],
        };
        state.groups.insert(gid.clone(), group);
        state.players.get_mut(&addr).unwrap().group = Some(gid.clone());

        let _ = tx.send(GlobalEvent {
            sender_addr: Some(addr),
            message: format!("S: EVT GROUP CREATED {}\n", username),
            target_room: None,
            target_group: Some(gid.clone()),
            target_player: None,
        });
        "S: OK group_created\n".to_string()
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_group_invite(addr: SocketAddr, target: String, state: &mut ServerState, tx: &Sender<GlobalEvent>) -> String {
    // Check that the inviter has a group
    let (gid, inviter_name) = if let Some(player) = state.players.get(&addr) {
        match &player.group {
            Some(g) => (g.clone(), player.username.clone()),
            None => return "S: ERR you_have_no_group\n".to_string(),
        }
    } else {
        return "S: ERR utilize_connect_first\n".to_string();
    };

    // Find the target player by username
    let target_addr = state.players.iter()
        .find(|(_, p)| ci_eq(&p.username, &target))
        .map(|(a, _)| *a);

    let target_addr = match target_addr {
        Some(a) => a,
        None => return "S: ERR player_not_found\n".to_string(),
    };

    if target_addr == addr {
        return "S: ERR cannot_invite_yourself\n".to_string();
    }

    // Check that the target doesn't already have a group
    if let Some(target_player) = state.players.get(&target_addr) {
        if target_player.group.is_some() {
            return "S: ERR player_already_in_group\n".to_string();
        }
    }

    // Add pending group invitation
    let target_username = state.players.get(&target_addr).unwrap().username.clone();

    state.players.get_mut(&target_addr).unwrap().pending_group_invite = Some(gid.clone());

    let _ = tx.send(GlobalEvent {
        sender_addr: Some(addr),
        message: format!("S: EVT GROUP INVITED {} invited you to join their group (type 'group accept' to join)\n", inviter_name),
        target_room: None,
        target_group: None,
        target_player: Some(target_addr),
    });
    format!("S: OK you invited {}\n", target_username)
}

fn handle_group_accept(addr: SocketAddr, state: &mut ServerState, tx: &Sender<GlobalEvent>) -> String {
    let (pending_gid, username, pid) = if let Some(player) = state.players.get(&addr) {
        if player.group.is_some() {
            return "S: ERR you_already_have_a_group\n".to_string();
        }
        match &player.pending_group_invite {
            Some(g) => (g.clone(), player.username.clone(), player.id.clone()),
            None => return "S: ERR no_pending_invite\n".to_string(),
        }
    } else {
        return "S: ERR utilize_connect_first\n".to_string();
    };

    // Check if the group still exists
    if let Some(group) = state.groups.get_mut(&pending_gid) {
        group.members.push(pid);
        let p = state.players.get_mut(&addr).unwrap();
        p.group = Some(pending_gid.clone());
        p.pending_group_invite = None;
        
        let _ = tx.send(GlobalEvent {
            sender_addr: Some(addr),
            message: format!("S: EVT GROUP JOIN {}\n", username),
            target_room: None,
            target_group: Some(pending_gid),
            target_player: None,
        });
        "S: OK you_joined_the_group\n".to_string()
    } else {
        state.players.get_mut(&addr).unwrap().pending_group_invite = None;
        "S: ERR group_no_longer_exists\n".to_string()
    }
}

fn handle_group_leave(addr: SocketAddr, state: &mut ServerState, tx: &Sender<GlobalEvent>) -> String {
    let (gid, username) = if let Some(player) = state.players.get(&addr) {
        match &player.group {
            Some(g) => (g.clone(), player.username.clone()),
            None => return "S: ERR you_have_no_group\n".to_string(),
        }
    } else {
        return "S: ERR utilize_connect_first\n".to_string();
    };

    let player_id = PlayerId(username.clone());

    // Remove from group
    state.players.get_mut(&addr).unwrap().group = None;

    let disband = if let Some(group) = state.groups.get_mut(&gid) {
        group.members.retain(|m| m != &player_id);
        group.members.len() <= 1
    } else {
        true
    };

    if disband {
        // If 1 or 0 members remain, disband the group
        if let Some(group) = state.groups.remove(&gid) {
            for member_id in &group.members {
                // Remove group_id from the last remaining member
                if let Some((_, p)) = state.players.iter_mut().find(|(_, p)| &p.id == member_id) {
                    p.group = None;
                }
            }
        }
        let _ = tx.send(GlobalEvent {
            sender_addr: Some(addr),
            message: format!("S: EVT GROUP DISBAND {} left, group disbanded\n", username),
            target_room: None,
            target_group: Some(gid.clone()), // The members might still get it if they haven't been removed yet
            target_player: None,
        });
        "S: OK group_disbanded\n".to_string()
    } else {
        let _ = tx.send(GlobalEvent {
            sender_addr: Some(addr),
            message: format!("S: EVT GROUP LEAVE {}\n", username),
            target_room: None,
            target_group: Some(gid),
            target_player: None,
        });
        "S: OK you_left_the_group\n".to_string()
    }
}

fn handle_group_info(addr: SocketAddr, state: &ServerState) -> String {
    if let Some(player) = state.players.get(&addr) {
        match &player.group {
            None => "S: OK No group\n".to_string(),
            Some(gid) => {
                if let Some(group) = state.groups.get(gid) {
                    let members: Vec<String> = group.members.iter().map(|pid| {
                        state.players.values()
                            .find(|p| &p.id == pid)
                            .map(|p| p.username.clone())
                            .unwrap_or_else(|| pid.0.clone())
                    }).collect();
                    format!("S: OK Group members: [{}]\n", members.join(", "))
                } else {
                    "S: OK No group\n".to_string()
                }
            }
        }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_quests(addr: SocketAddr, state: &ServerState, world: &WorldData) -> String {
    let Some(player) = state.players.get(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };

    if player.quest_progress.is_empty() {
        return "S: EVT QUEST_DATA empty\n".to_string();
    }

    let mut parts = Vec::new();
    for prog in &player.quest_progress {
        if let Some(q) = world.world.quests.iter().find(|q| q.id == prog.quest_id) {
            // Pick the objective text to display for the current stage
            let objective_str = if !q.stages.is_empty() {
                if let Some(stage_num) = prog.stage.stage_num() {
                    if let Some(stage_def) = q.stages.iter().find(|s| s.stage == stage_num) {
                        if let Some(obj) = &stage_def.objective {
                            match obj {
                                domain::quest::StageObjective::TalkToNpc { npc } => format!("Parler a {} (etape {})", npc, stage_num),
                                domain::quest::StageObjective::FetchItem { item } => format!("Recuperer {} (etape {})", item, stage_num),
                                domain::quest::StageObjective::KillMob { npc, count } => format!("Tuer {} x{} ({}/{}) (etape {})", npc, count, prog.counter, count, stage_num),
                                domain::quest::StageObjective::AnswerDialogue { npc, .. } => format!("Repondre a {} (etape {})", npc, stage_num),
                            }
                        } else { format!("Etape {}", stage_num) }
                    } else { format!("Etape {}", stage_num) }
                } else { prog.stage.to_wire() }
            } else {
                match &q.objective {
                    crate::world::QuestObjective::FetchItem { item } => format!("Recuperer {}", item),
                    crate::world::QuestObjective::DefeatNpc { npc } => format!("Vaincre {}", npc),
                    crate::world::QuestObjective::DeliverItem { item, to } => format!("Livrer {} a {}", item, to),
                    crate::world::QuestObjective::DeliverItems { items, to } => format!("Livrer [{}] a {}", items.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", "), to),
                    crate::world::QuestObjective::TalkToNpc { npc } => format!("Parler a {}", npc),
                }
            };
            parts.push(format!("{}:{}:{}:{}:{}", q.id, q.name, q.description, objective_str, prog.stage.to_wire()));
        }
    }

    if parts.is_empty() {
        "S: EVT QUEST_DATA empty\n".to_string()
    } else {
        format!("S: EVT QUEST_DATA {}\n", parts.join("|"))
    }
}
