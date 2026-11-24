//! Inventory commands: `INVENTORY`, `INFO`, `EQUIPMENT`, `TAKE`, `DROP`, `USE`, `STATUP`, `STATRESET`.

use std::net::SocketAddr;
use serde_json::json;
use tokio::sync::broadcast::Sender;

use crate::state::ServerState;
use domain::CombatState;
use crate::world::WorldData;
use crate::events::{to_player, to_room, GlobalEvent};
use crate::logging::log_event;
use crate::handlers::player_events::{get_char_stats_event_with_world, get_equip_data_event, get_inv_data_event, player_stats_event};
use crate::handlers::quest_core::quest_item_collected;

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
    // Among identical items, take the closest one this player can still collect.
    let mut item_is_here = false;
    let mut closest: Option<(usize, f32)> = None;
    for (index, ri) in items_in_room.iter().enumerate() {
        if ri.item_id != item.id {
            continue;
        }
        item_is_here = true;
        if quest_item_collected(player, ri, world) {
            continue;
        }
        let distance = (ri.pos_x - player.pos_x).powi(2) + (ri.pos_y - player.pos_y).powi(2);
        if closest.is_none_or(|(_, best)| distance < best) {
            closest = Some((index, distance));
        }
    }
    let Some((index, _)) = closest else {
        return if item_is_here {
            "S: ERR You have already taken this quest item.\n".to_string()
        } else {
            "S: ERR This item is not here.\n".to_string()
        };
    };
    let uid = items_in_room[index].uid;
    let is_quest_item = item.r#type == crate::world::ItemType::Quest;
    if is_quest_item {
        items_in_room[index].collected_by.insert(player.username.clone());
    } else {
        items_in_room.remove(index);
    }

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
    // A quest item only disappears for its collector: others can still take their copy.
    let despawn = format!("S: EVT ROOM ITEM_DESPAWN {} {}\n", item.id, uid);
    let own_despawn = if is_quest_item {
        despawn
    } else {
        to_room(tx, None, despawn, current_room_id);
        String::new()
    };
    format!(
        "S: OK taken={}{}\n{}{}{}",
        item.id,
        quest_msg,
        own_despawn,
        get_inv_data_event(player, world),
        player_stats_event(player)
    )
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
            state.next_item_uid += 1;
            let uid = state.next_item_uid;
            state
                .room_items
                .entry(current_room_id.clone())
                .or_default()
                .push(crate::state::RuntimeItem {
                    uid,
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
                    "S: EVT ROOM ITEM_SPAWN {} {} {} {} {} {}\n",
                    item_id, item_name, sprite, drop_x as i32, drop_y as i32, uid
                ),
                current_room_id,
            );
            format!(
                "S: OK dropped={}\n{}",
                item_id,
                get_inv_data_event(player, world)
            )
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
    // Keep the item: healing at full HP would waste it.
    if player.hp >= player.max_hp {
        return "S: ERR Your HP is already full!\n".to_string();
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

/// Refunds every stat point spent with STATUP: base stats go back to 0,
/// max HP back to its base value, and the points become spendable again.
pub(crate) fn handle_statreset(
    addr: SocketAddr,
    state: &mut ServerState,
    world: &WorldData,
) -> String {
    let Some(player) = state.players.get_mut(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    // Changing max HP mid-fight would be an exploit.
    if matches!(player.combat, CombatState::InCombat { .. }) {
        return "S: ERR cannot_reset_in_combat\n".to_string();
    }
    let hp_points = (player.max_hp - player.base_max_hp).max(0) / 5;
    let refunded = player.stats.damage
        + player.stats.special_damage
        + player.stats.defense
        + player.stats.special_defense
        + hp_points;
    if refunded <= 0 {
        return "S: ERR nothing_to_reset\n".to_string();
    }
    player.stats.damage = 0;
    player.stats.special_damage = 0;
    player.stats.defense = 0;
    player.stats.special_defense = 0;
    player.max_hp = player.base_max_hp;
    player.hp = player.hp.min(player.max_hp);
    player.stat_points += refunded;
    let char_stats = get_char_stats_event_with_world(player, world);
    let stats_evt = player_stats_event(player);
    format!(
        "S: OK Stats reset! {} points refunded ({} points available)\n{}{}",
        refunded, player.stat_points, stats_evt, char_stats
    )
}
