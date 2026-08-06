use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use rand::Rng;
use serde_json::json;
use tokio::sync::broadcast::Sender;

use domain::{xp_required_for_level, Armor, Direction, Equipement, Group, GroupId, ItemBucket, ItemId, PlayerId, RoomId, Weapon, WeaponType};

use crate::commands::GameCommand;
use crate::state::{Player, ServerState, START_ROOM};
use crate::world::{Item as CatalogItem, WorldData};
use crate::{log_event, DbPool, GlobalEvent};

fn ci_eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn make_weapon(item: &CatalogItem) -> Weapon {
    Weapon {
        id: item.id.clone(),
        name: item.name.clone(),
        damages: item.damage.unwrap_or(0),
        category: WeaponType::Melee,
    }
}

/// Construit une `Armor` de domaine à partir d'un objet du catalogue.
fn make_armor(item: &CatalogItem) -> Armor {
    Armor {
        id: item.id.clone(),
        name: item.name.clone(),
        description: item.description.clone(),
        defense: item.defense.unwrap_or(0),
        special_defense: 0,
    }
}

/// Reconstruit l'équipement structuré depuis la table emplacement -> id (BDD),
/// en relisant les caractéristiques dans le catalogue.
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

// Point d'entrée en async

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
        GameCommand::Talk(c)        => (handle_talk(addr, c, state, world), false),
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
        GameCommand::Quit           => ("S: OK goodbye\n".to_string(), true),
        GameCommand::Unknown        => ("S: ERR malformed_command\n".to_string(), false),
        _                           => ("S: OK command received but not implemented yet\n".to_string(), false),
    }
}

// Save appelé a la deconnexion

pub async fn save_player_to_db(pool: &DbPool, player: &Player) {
    // L'équipement structuré est aplati en table emplacement -> id pour la BDD.
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
    ).await;

    if let Err(e) = result {
        eprintln!("[DB] Error saving player {}: {}", player.username, e);
    } else {
        println!("[DB] Player {} saved.", player.username);
    }
}

// Connect adapté pour le BDD

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

    // Empêche de se connecter deux fois sur le même compte
    if state.players.values().any(|p| p.username == pseudo) {
        return "S: ERR already_logged_in\n".to_string();
    }

    // Essaie de charger un joueur existant
    match database::load_player(pool, &pseudo).await {
        Ok(Some(data)) => {
            // Compte existant → on vérifie le mot de passe avant de restaurer
            match database::verify_player(pool, &pseudo, &password).await {
                Ok(true) => {} // mot de passe correct, on continue
                Ok(false) => {
                    log_event("AUTH_FAIL", &pseudo, json!({"ip": addr.to_string()}));
                    return "S: ERR bad_credentials\n".to_string();
                }
                Err(e) => {
                    eprintln!("[DB] Verification error: {}", e);
                    return "S: ERR internal_error\n".to_string();
                }
            }

            // Joueur trouvé → restaurer son état dans le modèle enrichi
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

            // On capture le skin avant que `player` ne soit déplacé dans l'état :
            // le client s'en sert pour afficher le bon avatar.
            let skin = player.skin.clone();
            let enter_room = player.current_room.clone();
            state.players.insert(addr, player);
            log_event("CONNECT", &pseudo, json!({"ip": addr.to_string(), "type": "returning"}));
            let _ = tx.send(GlobalEvent {
                sender_addr: addr,
                message: format!("S: EVT GLOBAL CHAT Server {} is back!\n", pseudo),
                target_room: None,
                target_group: None,
            target_player: None,
            });
            let _ = tx.send(GlobalEvent {
                sender_addr: addr,
                message: format!("S: EVT ROOM {} PRESENCE ENTER {} {} 0 0\n", enter_room, pseudo, skin),
                target_room: Some(enter_room.clone()),
                target_group: None,
            target_player: None,
            });
            let exits = format_exits(world, &enter_room);
            format!(
                "S: OK connected skin={} name={} room={}{}\n{}",
                skin, pseudo, enter_room, exits,
                room_presence_roster(addr, state, &enter_room)
            )
        }
        Ok(None) => {
            // Nouveau joueur -> on crée le compte avec le mot de passe fourni
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
                sender_addr: addr,
                message: format!("S: EVT GLOBAL CHAT Server {} just connected!\n", pseudo),
                target_room: None,
                target_group: None,
            target_player: None,
            });
            let _ = tx.send(GlobalEvent {
                sender_addr: addr,
                message: format!("S: EVT ROOM {} PRESENCE ENTER {} {} 0 0\n", enter_room, pseudo, skin),
                target_room: Some(enter_room.clone()),
                target_group: None,
            target_player: None,
            });
            let exits = format_exits(world, &enter_room);
            format!(
                "S: OK connected skin={} name={} room={}{}\n{}",
                skin, pseudo, enter_room, exits,
                room_presence_roster(addr, state, &enter_room)
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
            sender_addr: addr,
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
                "S: EVT ROOM {} PRESENCE ENTER {} {} {} {}\n",
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
        sender_addr: addr,
        message: format!("S: EVT ROOM {} PRESENCE LEAVE {}\n", old_room, username),
        target_room: Some(old_room.clone()),
        target_group: None,
            target_player: None,
    });
    let _ = tx.send(GlobalEvent {
        sender_addr: addr,
        message: format!("S: EVT ROOM {} PRESENCE ENTER {} {} {} {}\n", next_room, username, skin, px as i64, py as i64),
        target_room: Some(next_room.clone()),
        target_group: None,
            target_player: None,
    });
    let exits = format_exits(world, &next_room);
    format!("S: OK room-loc.{}{}\n{}", next_room, exits, room_presence_roster(addr, state, &next_room))
}

fn get_inv_data_event(player: &crate::state::Player, world: &WorldData) -> String {
    if player.inventory.is_empty() {
        return "S: EVT INV_DATA empty\n".to_string();
    }
    let mut data_objets = Vec::new();
    for (id, count) in player.inventory.iter() {
        let item_opt = world.world.items.iter().find(|i| &i.id == id);
        let name = item_opt.map(|i| i.name.clone()).unwrap_or_else(|| id.to_string());
        let slot = item_opt.and_then(|i| i.slot.clone()).unwrap_or_else(|| "none".to_string());
        let damage = item_opt.and_then(|i| i.damage).unwrap_or(0);
        data_objets.push(format!("{}:{}:{}:{}:{}", id, count, slot, name, damage));
    }
    format!("S: EVT INV_DATA {}\n", data_objets.join("|"))
}

fn get_equip_data_event(player: &crate::state::Player) -> String {
    let head = player.equipement.helmet.as_ref().map(|a| format!("{}:{}:{}", a.id, a.name, a.defense)).unwrap_or_else(|| "none".to_string());
    let chest = player.equipement.chestplate.as_ref().map(|a| format!("{}:{}:{}", a.id, a.name, a.defense)).unwrap_or_else(|| "none".to_string());
    let legs = player.equipement.legging.as_ref().map(|a| format!("{}:{}:{}", a.id, a.name, a.defense)).unwrap_or_else(|| "none".to_string());
    let weapon = player.equipement.weapon.as_ref().map(|w| format!("{}:{}:{}", w.id, w.name, w.damages)).unwrap_or_else(|| "none".to_string());
    format!("S: EVT EQUIP_DATA head:{}|chest:{}|legs:{}|weapon:{}\n", head, chest, legs, weapon)
}

fn handle_inventory(addr: SocketAddr, state: &ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get(&addr) {
        if player.inventory.is_empty() {
            format!("S: OK Your inventory is empty.\n{}{}", get_inv_data_event(player, world), get_equip_data_event(player))
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
            format!("S: OK Inventory: [{}]\n{}{}", noms_objets.join(", "), get_inv_data_event(player, world), get_equip_data_event(player))
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

            if slot == "weapon" {
                let old = player.equipement.weapon.take();
                player.equipement.weapon = Some(make_weapon(item));
                if let Some(old) = old { player.inventory.add(old.id, ItemBucket::Weapon); }
                player.inventory.remove_one(&item.id);
                format!("S: OK You equipped {} in slot [{}].\n{}{}", item.name, slot, get_inv_data_event(player, world), get_equip_data_event(player))
            } else if let Some(slot_ref) = player.equipement.armor_slot_mut(&slot) {
                let old = slot_ref.take();
                *slot_ref = Some(make_armor(item));
                if let Some(old) = old { player.inventory.add(old.id, ItemBucket::Item); }
                player.inventory.remove_one(&item.id);
                format!("S: OK You equipped {} in slot [{}].\n{}{}", item.name, slot, get_inv_data_event(player, world), get_equip_data_event(player))
            } else {
                "S: ERR This item cannot be equipped.\n".to_string()
            }
        } else { "S: ERR Unknown item.\n".to_string() }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_unequip(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get_mut(&addr) {
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
            format!("S: OK You unequipped {}.\n{}{}", name, get_inv_data_event(player, world), get_equip_data_event(player))
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
        format!("S: OK --- EQUIPMENT ---\n  Head: {}\n  Chest: {}\n  Legs: {}\n  Weapon: {}\n{}", head, chest, legs, weapon, get_equip_data_event(player))
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_take(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get_mut(&addr) {
        let salle_actuelle = player.current_room.clone();
        if let Some(item) = world.world.items.iter().find(|i| ci_eq(i.id.as_str(), &cible) || ci_eq(&i.name, &cible)) {
            if let Some(items_salle) = state.room_items.get_mut(&salle_actuelle) {
                if let Some(index) = items_salle.iter().position(|ri| ri.item_id == item.id) {
                    let mut deja_ramasse = false;
                    match item.r#type {
                        crate::world::ItemType::Standard => { items_salle.remove(index); }
                        crate::world::ItemType::Quest => {
                            let runtime_item = &mut items_salle[index];
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
                    let salle_actuelle = player.current_room.clone();
                    state.room_items.entry(salle_actuelle).or_default().push(crate::state::RuntimeItem {
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

fn handle_talk(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData) -> String {
    let mut npc_trouve = None;
    let mut quete_validee = false;
    let mut quest_msg = String::new();
    if let Some(player) = state.players.get_mut(&addr) {
        let salle_actuelle = player.current_room.clone();
        if let Some(npcs_ici) = state.room_npcs.get(&salle_actuelle) {
            for npc_id in npcs_ici {
                if let Some(npc) = world.world.npcs.iter().find(|n| &n.id == npc_id) {
                    if ci_eq(npc.id.as_str(), &cible) || ci_eq(&npc.name, &cible) {
                        npc_trouve = Some(npc.clone()); break;
                    }
                }
            }
        }
        if let Some(npc) = &npc_trouve {
            for q in &world.world.quests {
                if let crate::world::QuestObjective::DeliverItem { .. } = &q.objective {
                    if q.giver_id.as_ref() == Some(&npc.id) && player.inventory.contains(&q.target_id) && !player.completed_quests.contains(&q.id) {
                        player.completed_quests.push(q.id.clone());
                        player.inventory.remove_one(&q.target_id);
                        if let Some(ref reward_id) = q.reward_item {
                            let bucket = world.world.items.iter().find(|i| &i.id == reward_id).map(|i| i.bucket()).unwrap_or(ItemBucket::Item);
                            player.inventory.add(reward_id.clone(), bucket);
                            let item_name = world.world.items.iter().find(|i| &i.id == reward_id).map(|i| i.name.clone()).unwrap_or_else(|| reward_id.to_string());
                            quest_msg = format!("\n[QUEST COMPLETE] {}! You receive: {}.", q.name, item_name);
                        }
                        if let Some(xp) = q.reward_exp { player.add_xp(xp); quest_msg.push_str(&format!(" (+{} EXP)", xp)); }
                        quete_validee = true; break;
                    }
                }
            }
        }
    }
    if let Some(npc) = npc_trouve {
        if quete_validee { format!("S: OK {} takes the item. {}\n", npc.name, quest_msg) }
        else { format!("S: OK {} says: \"{}\"\n", npc.name, npc.dialogue.join(" ")) }
    } else { "S: ERR There is nobody by that name here.\n".to_string() }
}

fn handle_attack(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData, tx: &Sender<GlobalEvent>) -> String {
    let mut salle_actuelle = RoomId(String::new());
    let mut degats_joueur = 10;
    let mut armure_joueur = 0;
    let mut monstre_id_trouve = None;
    let mut monstre_nom = String::new();
    let mut joueur_nom = String::new();
    let mut degats_monstre = 5;
    let mut defense_monstre = 0;
    let mut exp_gagnee = 0;
    let mut drops_monstre = Vec::new();
    let mut en_cooldown = false;

    if let Some(player) = state.players.get(&addr) {
        if player.last_attack.map_or(false, |last| last.elapsed() < Duration::from_millis(1000)) { en_cooldown = true; }
    }
    if en_cooldown { return "S: ERR attack_cooldown\n".to_string(); }

    if let Some(player) = state.players.get_mut(&addr) {
        salle_actuelle = player.current_room.clone();
        joueur_nom = player.username.clone();
        player.last_attack = Some(Instant::now());
        // Dégâts = base + arme équipée ; armure = somme des pièces équipées.
        degats_joueur += player.equipement.weapon_damage();
        armure_joueur += player.equipement.total_defense();
    } else { return "S: ERR utilize_connect_first\n".to_string(); }

    if let Some(npcs_dans_salle) = state.room_npcs.get(&salle_actuelle) {
        for npc_id in npcs_dans_salle {
            if let Some(npc) = world.world.npcs.iter().find(|n| &n.id == npc_id) {
                if npc.role == "enemy" && (ci_eq(npc.id.as_str(), &cible) || ci_eq(&npc.name, &cible)) {
                    monstre_id_trouve = Some(npc.id.clone());
                    monstre_nom = npc.name.clone();
                    degats_monstre = npc.damage.unwrap_or(5);
                    defense_monstre = npc.defense.unwrap_or(0);
                    exp_gagnee = npc.exp_reward.unwrap_or(0);
                    drops_monstre = npc.drops.clone();
                    break;
                }
            }
        }
    }

    if let Some(m_id) = monstre_id_trouve {
        let mut monstre_mort = false;
        let mut pv_monstre_restants = 0;
        let degats_finaux = (degats_joueur - defense_monstre).max(1);
        if let Some(hp) = state.npc_hps.get_mut(&m_id) {
            *hp -= degats_finaux;
            pv_monstre_restants = *hp;
            if *hp <= 0 { monstre_mort = true; }
        }
        if monstre_mort {
            if let Some(npcs_dans_salle) = state.room_npcs.get_mut(&salle_actuelle) {
                npcs_dans_salle.retain(|id| id != &m_id);
            }
            if let Some(npc_static) = world.world.npcs.iter().find(|n| n.id == m_id) {
                let delay = npc_static.respawn_time.unwrap_or(30);
                state.dead_npcs.push(crate::state::DeadNpc {
                    npc_id: m_id.clone(), room_id: salle_actuelle.clone(),
                    respawn_at: Instant::now() + Duration::from_secs(delay),
                });
            }
            let mut noms_drops = Vec::new();
            if let Some(player) = state.players.get_mut(&addr) {
                player.add_xp(exp_gagnee);
                for drop in drops_monstre {
                    if rand::thread_rng().gen_range(1..=100) <= drop.chance {
                        let bucket = world.world.items.iter().find(|i| i.id == drop.item_id).map(|i| i.bucket()).unwrap_or(ItemBucket::Item);
                        player.inventory.add(drop.item_id.clone(), bucket);
                        let nom = world.world.items.iter().find(|i| i.id == drop.item_id).map(|i| i.name.clone()).unwrap_or_else(|| drop.item_id.to_string());
                        noms_drops.push(nom);
                    }
                }
            }
            let mut msg = format!("S: OK You dealt {} damage. {} collapses! (+{} EXP)\n", degats_finaux, monstre_nom, exp_gagnee);
            if !noms_drops.is_empty() { msg.push_str(&format!("You obtain: {}\n", noms_drops.join(", "))); }
            msg
        } else {
            let mut joueur_mort = false;
            let mut pv_joueur = 0;
            let degats_monstre_finaux = (degats_monstre - armure_joueur).max(1);
            if let Some(player) = state.players.get_mut(&addr) {
                player.hp -= degats_monstre_finaux;
                pv_joueur = player.hp;
                if player.hp <= 0 {
                    joueur_mort = true;
                    player.hp = player.max_hp;
                    player.current_room = RoomId::from(START_ROOM);
                }
            }
            if joueur_mort {
                let _ = tx.send(GlobalEvent { sender_addr: addr, message: format!("S: EVT GLOBAL CHAT Server A player was killed by {}!\n", monstre_nom), target_room: None, target_group: None, target_player: None });
                log_event("DEATH", &joueur_nom, json!({"killer": monstre_nom}));
                format!("S: OK You deal {} damage, but {} finishes you off. You are DEAD! You wake up at the oasis.\n", degats_finaux, monstre_nom)
            } else {
                format!("S: OK You attack {} ({} HP left). It retaliates (-{} HP). (Your HP: {})\n", monstre_nom, pv_monstre_restants, degats_monstre_finaux, pv_joueur)
            }
        }
    } else { "S: ERR You can't attack that.\n".to_string() }
}

fn handle_chat(addr: SocketAddr, channel: String, message: String, state: &mut ServerState, tx: &Sender<GlobalEvent>) -> String {
    let mut en_cooldown = false;
    let mut current_room = String::new();
    if let Some(player) = state.players.get(&addr) {
        if player.last_chat.map_or(false, |last| last.elapsed() < Duration::from_millis(2000)) { en_cooldown = true; }
        else { current_room = player.current_room.as_str().to_string(); }
    }
    if en_cooldown { return "S: ERR chat_spam_forbidden\n".to_string(); }
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
                let _ = tx.send(GlobalEvent { sender_addr: addr, message: format!("S: EVT GLOBAL CHAT {} {}\n", username, message), target_room: None, target_group: None, target_player: None });
                "S: OK\n".to_string()
            }
            "ROOM" => {
                let _ = tx.send(GlobalEvent { sender_addr: addr, message: format!("S: EVT ROOM {} CHAT {} {}\n", current_room, username, message), target_room: Some(RoomId::from(current_room.as_str())), target_group: None, target_player: None });
                "S: OK\n".to_string()
            }
            "GROUP" => {
                let _ = tx.send(GlobalEvent { sender_addr: addr, message: format!("S: EVT GROUP CHAT {} {}\n", username, message), target_room: None, target_group: group_id_opt, target_player: None });
                "S: OK\n".to_string()
            }
            _ => "S: ERR unknown_channel\n".to_string(),
        }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_who(addr: SocketAddr, state: &ServerState) -> String {
    let total_serveur = state.players.len();
    if let Some(current_player) = state.players.get(&addr) {
        let joueurs_piece: Vec<String> = state.players.values()
            .filter(|p| p.current_room == current_player.current_room)
            .map(|p| format!("\"{}\"", p.username))
            .collect();
        format!("S: OK {{ \"room\": [{}], \"server\": {} }}\n", joueurs_piece.join(", "), total_serveur)
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
            sender_addr: addr,
            message: format!("S: EVT GROUP CREATED {}\n", username),
            target_room: None,
            target_group: Some(gid.clone()),
            target_player: None,
        });
        "S: OK group_created\n".to_string()
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_group_invite(addr: SocketAddr, target: String, state: &mut ServerState, tx: &Sender<GlobalEvent>) -> String {
    // Vérifier que l'inviteur a un groupe
    let (gid, inviter_name) = if let Some(player) = state.players.get(&addr) {
        match &player.group {
            Some(g) => (g.clone(), player.username.clone()),
            None => return "S: ERR you_have_no_group\n".to_string(),
        }
    } else {
        return "S: ERR utilize_connect_first\n".to_string();
    };

    // Trouver le joueur cible par son username
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

    // Vérifier que la cible n'a pas déjà un groupe
    if let Some(target_player) = state.players.get(&target_addr) {
        if target_player.group.is_some() {
            return "S: ERR player_already_in_group\n".to_string();
        }
    }

    // Ajouter au groupe (invitation en attente)
    let target_username = state.players.get(&target_addr).unwrap().username.clone();

    state.players.get_mut(&target_addr).unwrap().pending_group_invite = Some(gid.clone());

    let _ = tx.send(GlobalEvent {
        sender_addr: addr,
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

    // Vérifier si le groupe existe toujours
    if let Some(group) = state.groups.get_mut(&pending_gid) {
        group.members.push(pid);
        let p = state.players.get_mut(&addr).unwrap();
        p.group = Some(pending_gid.clone());
        p.pending_group_invite = None;
        
        let _ = tx.send(GlobalEvent {
            sender_addr: addr,
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

    // Retirer du groupe
    state.players.get_mut(&addr).unwrap().group = None;

    let disband = if let Some(group) = state.groups.get_mut(&gid) {
        group.members.retain(|m| m != &player_id);
        group.members.len() <= 1
    } else {
        true
    };

    if disband {
        // S'il reste 1 ou 0 membre, on dissout le groupe
        if let Some(group) = state.groups.remove(&gid) {
            for member_id in &group.members {
                // Retirer le group_id du dernier membre restant
                if let Some((_, p)) = state.players.iter_mut().find(|(_, p)| &p.id == member_id) {
                    p.group = None;
                }
            }
        }
        let _ = tx.send(GlobalEvent {
            sender_addr: addr,
            message: format!("S: EVT GROUP DISBAND {} left, group disbanded\n", username),
            target_room: None,
            target_group: Some(gid.clone()), // The members might still get it if they haven't been removed yet
            target_player: None,
        });
        "S: OK group_disbanded\n".to_string()
    } else {
        let _ = tx.send(GlobalEvent {
            sender_addr: addr,
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
