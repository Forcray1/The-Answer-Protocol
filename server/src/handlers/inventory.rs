use std::net::SocketAddr;
use serde_json::json;
use tokio::sync::broadcast::Sender;

use crate::state::ServerState;
use crate::world::WorldData;
use crate::events::{to_player, to_room, GlobalEvent};
use crate::logging::log_event;
use crate::handlers::player_events::{get_char_stats_event_with_world, get_equip_data_event, get_inv_data_event, player_stats_event};

pub(crate) fn handle_inventory(addr: SocketAddr, state: &ServerState, world: &WorldData) -> String {
    let Some(player) = state.players.get(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    // RFC 42TAP: OK ["item.herbs", "item.bread"] (one entry per unit held)
    let mut ids: Vec<String> = Vec::new();
    for (id, count) in player.inventory.iter() {
        for _ in 0..*count {
            ids.push(id.to_string());
        }
    }
    ids.sort();
    format!(
        "S: OK {}\n{}{}{}",
        json!(ids),
        get_inv_data_event(player, world),
        get_equip_data_event(player, world),
        get_char_stats_event_with_world(player, world)
    )
}

pub(crate) fn handle_info(addr: SocketAddr, cible: String, state: &ServerState, world: &WorldData) -> String {
    let Some(player) = state.players.get(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    let Some(item) = world.find_item(&cible) else {
        return "S: ERR Unknown item.\n".to_string();
    };
    if player.inventory.contains(&item.id) {
        let mut stats = String::new();
        if let Some(dmg) = item.damage {
            stats.push_str(&format!("\n Damage: +{}", dmg));
        }
        if let Some(def) = item.defense {
            stats.push_str(&format!("\n Armor: +{}", def));
        }
        format!(
            "S: OK --- {} ---\n {}{}\n",
            item.name, item.description, stats
        )
    } else {
        "S: ERR You don't have this item.\n".to_string()
    }
}

pub(crate) fn handle_equipment(addr: SocketAddr, state: &ServerState, world: &WorldData) -> String {
    let Some(player) = state.players.get(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    let none_value = || "None".to_string();
    let chest = player
        .equipement
        .chestplate
        .as_ref()
        .map(|a| a.name.clone())
        .unwrap_or_else(none_value);
    let weapon = player
        .equipement
        .weapon
        .as_ref()
        .map(|w| w.name.clone())
        .unwrap_or_else(none_value);
    format!(
        "S: OK --- EQUIPMENT ---\n  Chest: {}\n  Weapon: {}\n{}{}",
        chest,
        weapon,
        get_equip_data_event(player, world),
        get_char_stats_event_with_world(player, world)
    )
}

pub(crate) fn handle_take(
    addr: SocketAddr,
    cible: String,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    let Some(player) = state.players.get_mut(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    let current_room_id = player.current_room.clone();
    let Some(item) = world.find_item(&cible) else {
        return "S: ERR Unknown item.\n".to_string();
    };
    let Some(items_in_room) = state.room_items.get_mut(&current_room_id) else {
        return "S: ERR room_error\n".to_string();
    };
    let Some(index) = items_in_room.iter().position(|ri| ri.item_id == item.id) else {
        return "S: ERR This item is not here.\n".to_string();
    };
    let mut deja_ramasse = false;
    match item.r#type {
        crate::world::ItemType::Standard => {
            items_in_room.remove(index);
        }
        crate::world::ItemType::Quest => {
            let runtime_item = &mut items_in_room[index];
            if runtime_item.collected_by.contains(&player.username) {
                deja_ramasse = true;
            } else {
                runtime_item.collected_by.insert(player.username.clone());
            }
        }
    }
    if deja_ramasse {
        "S: ERR You have already taken this quest item.\n".to_string()
    } else {
        player.inventory.add(item.id.clone(), item.bucket());
        log_event("TAKE", &player.username, json!({"item_id": item.id}));
        let mut quest_msg = String::new();
        for q in &world.world.quests {
            if let crate::world::QuestObjective::FetchItem { item: target_id } =
                &q.objective {
                if target_id == &item.id && !player.completed_quests.contains(&q.id) {
                    player.completed_quests.push(q.id.clone());
                    if let Some(xp) = q.reward_exp {
                        player.add_xp_with_world(xp, world);
                        quest_msg =
                            format!(" [QUEST COMPLETE] {}! (+{} EXP)", q.name, xp);
                    }
                }
            }
        }
        // Broadcast despawn to all players in the room (standard items disappear).
        if item.r#type == crate::world::ItemType::Standard {
            to_room(
                tx,
                None,
                format!("S: EVT ROOM ITEM_DESPAWN {}\n", item.id),
                current_room_id,
            );
        }
        format!("S: OK taken={}{}\n", item.id, quest_msg)
    }
}

pub(crate) fn handle_drop(
    addr: SocketAddr,
    cible: String,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    let Some(player) = state.players.get_mut(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    let Some(item) = world.find_item(&cible) else {
        return "S: ERR Unknown item.\n".to_string();
    };
    if player.inventory.contains(&item.id) {
        if item.r#type == crate::world::ItemType::Quest {
            "S: ERR You cannot get rid of a quest item!\n".to_string()
        } else {
            let drop_x = player.pos_x;
            let drop_y = player.pos_y;
            player.inventory.remove_one(&item.id);
            let current_room_id = player.current_room.clone();
            let sprite = item.sprite.as_deref().unwrap_or("none").to_string();
            let item_name = item.name.replace(' ', "_");
            let item_id = item.id.clone();
            state
                .room_items
                .entry(current_room_id.clone())
                .or_default()
                .push(crate::state::RuntimeItem {
                    item_id: item_id.clone(),
                    source: crate::state::ItemSource::PlayerDrop,
                    collected_by: std::collections::HashSet::new(),
                    pos_x: drop_x,
                    pos_y: drop_y,
                });
            log_event("DROP", &player.username, json!({"item_id": item_id}));
            to_room(
                tx,
                Some(addr),
                format!(
                    "S: EVT ROOM CHAT {} [{}] has dropped {}\n",
                    player.username,
                    player.username,
                    item.name
                ),
                current_room_id.clone(),
            );
            // Broadcast spawn to all players in the room.
            to_room(
                tx,
                None,
                format!(
                    "S: EVT ROOM ITEM_SPAWN {} {} {} {} {}\n",
                    item_id, item_name, sprite, drop_x as i32, drop_y as i32
                ),
                current_room_id,
            );
            format!("S: OK dropped={}\n", item_id)
        }
    } else {
        format!("S: ERR You don't have the item \"{}\".\n", item.name)
    }
}

pub(crate) fn handle_use(
    addr: SocketAddr,
    item_id: String,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    let Some(player) = state.players.get_mut(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    let item_def = world.world.items.iter().find(|i| {
        i.id.as_str() == item_id
            || i.id
                .as_str()
                .replace('_', " ")
                .eq_ignore_ascii_case(&item_id)
    });
    let item_def = match item_def {
        Some(i) => i.clone(),
        None => return "S: ERR item_unknown\n".to_string(),
    };
    if !item_def.consumable {
        return "S: ERR item_not_consumable\n".to_string();
    }
    if !player.inventory.contains(&item_def.id) {
        return "S: ERR item_not_in_inventory\n".to_string();
    }
    player.inventory.remove_one(&item_def.id);
    let heal_pct = item_def.heal_percent.unwrap_or(50) as i32;
    let heal_amount = (player.max_hp * heal_pct / 100).max(1);
    let old_hp = player.hp;
    player.hp = (player.hp + heal_amount).min(player.max_hp);
    let actual_heal = player.hp - old_hp;
    let stats_evt = player_stats_event(player);
    let inv_evt = get_inv_data_event(player, world);
    to_player(
        tx,
        format!("{}{}", stats_evt, inv_evt),
        addr,
    );
    format!(
        "S: OK Used {}. Restored {} HP. (HP: {}/{})\n",
        item_def.name, actual_heal, player.hp, player.max_hp
    )
}

pub(crate) fn handle_statup(
    addr: SocketAddr,
    stat: String,
    state: &mut ServerState,
    _world: &WorldData,
) -> String {
    let Some(player) = state.players.get_mut(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    if player.stat_points <= 0 {
        return "S: ERR no_stat_points\n".to_string();
    }
    match stat.as_str() {
        "damage" => player.stats.damage += 1,
        "special_damage" => player.stats.special_damage += 1,
        "defense" => player.stats.defense += 1,
        "special_defense" => player.stats.special_defense += 1,
        "max_hp" => {
            player.max_hp += 5;
            player.hp = (player.hp + 5).min(player.max_hp);
        }
        _ => return "S: ERR unknown_stat\n".to_string(),
    }
    player.stat_points -= 1;
    let char_stats = get_char_stats_event_with_world(player, _world);
    let stats_evt = player_stats_event(player);
    format!(
        "S: OK Stat '{}' increased! ({} points left)\n{}{}",
        stat, player.stat_points, stats_evt, char_stats
    )
}
