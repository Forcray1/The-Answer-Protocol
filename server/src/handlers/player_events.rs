//! Wire-format builders for the player panels (inventory, equipment, stats) and attack totals.

use crate::world::WorldData;

pub(crate) fn is_bevy_inventory_slot(slot: &str) -> bool {
    matches!(
        slot,
        "weapon" | "chest" | "medaillon" | "key" | "none"
    )
}

pub(crate) fn get_inv_data_event(player: &crate::state::Player, world: &WorldData) -> String {
    if player.inventory.is_empty() {
        return "S: EVT INV_DATA empty\n".to_string();
    }
    let mut data_objets = Vec::new();
    for (id, count) in player.inventory.iter() {
        let item_opt = world.item_by_id(id);
        let slot = item_opt
            .and_then(|i| i.slot.clone())
            .unwrap_or_else(|| "none".to_string());
        if !is_bevy_inventory_slot(&slot) {
            continue;
        }

        let name = item_opt
            .map(|i| i.name.clone())
            .unwrap_or_else(|| id.to_string());
        let damage = item_opt.and_then(|i| i.damage).unwrap_or(0);
        let sprite = item_opt
            .and_then(|i| i.sprite.clone())
            .unwrap_or_else(|| "none".to_string());
        let consumable = if item_opt.map(|i| i.consumable).unwrap_or(false) {
            1u8
        } else {
            0u8
        };
        // Equipped items are never in the inventory, so the equipped flag is always 0.
        data_objets.push(format!(
            "{}:{}:{}:{}:{}:{}:0:{}",
            id, count, slot, name, damage, sprite, consumable
        ));
    }
    if data_objets.is_empty() {
        return "S: EVT INV_DATA empty\n".to_string();
    }
    format!("S: EVT INV_DATA {}\n", data_objets.join("|"))
}

pub(crate) fn get_equip_data_event(player: &crate::state::Player, world: &WorldData) -> String {
    let chest = player
        .equipement
        .chestplate
        .as_ref()
        .map(|a| {
            format!(
                "{}:{}:{}:{}",
                a.id,
                a.name,
                a.defense,
                a.sprite.as_deref().unwrap_or("none")
            )
        })
        .unwrap_or_else(|| "none".to_string());
    let weapon = player
        .equipement
        .weapon
        .as_ref()
        .map(|w| {
            format!(
                "{}:{}:{}:{}",
                w.id,
                w.name,
                w.damages,
                w.sprite.as_deref().unwrap_or("none")
            )
        })
        .unwrap_or_else(|| "none".to_string());
    let medaillon = player
        .equipped_medaillon_id
        .as_ref()
        .map(|mid| {
            let item = world.item_by_id(mid);
            let name = item.map(|i| i.name.clone()).unwrap_or_else(|| mid.to_string());
            let sprite = item.and_then(|i| i.sprite.as_deref()).unwrap_or("none");
            format!("{}:{}:0:{}", mid, name, sprite)
        })
        .unwrap_or_else(|| "none".to_string());
    format!(
        "S: EVT EQUIP_DATA chest:{}|weapon:{}|medaillon:{}\n",
        chest, weapon, medaillon
    )
}

pub(crate) fn player_physical_attack_total(player: &crate::state::Player, world: &WorldData) -> i32 {
    let base = player.stats.damage + player.equipement.weapon_damage();
    let mut total = base;

    if let Some(mid) = &player.equipped_medaillon_id {
        if let Some(item) = world.item_by_id(mid) {
            if item.id.as_str() == "strength_medaillon" {
                total += (base * 20) / 100;
            }
        }
    }

    total
}

pub(crate) fn player_special_attack_total(player: &crate::state::Player, world: &WorldData) -> i32 {
    let mut total = player.stats.special_damage;

    if let Some(weapon) = &player.equipement.weapon {
        if weapon.id.as_str() == "magic_staff" {
            total += 12;
        }
    }

    if let Some(mid) = &player.equipped_medaillon_id {
        if let Some(item) = world.item_by_id(mid) {
            if item.id.as_str() == "magical_strength_medaillon" {
                total += (player.stats.special_damage * 20) / 100;
            }
        }
    }

    total
}

pub(crate) fn get_char_stats_event_with_world(player: &crate::state::Player, world: &WorldData) -> String {
    let total_dmg = player_physical_attack_total(player, world);
    let total_spe_dmg = player_special_attack_total(player, world);
    let total_def = player.stats.defense + player.equipement.total_defense();
    let total_spe_def: i32 = player.stats.special_defense
        + player
            .equipement
            .chestplate
            .as_ref()
            .map(|a| a.special_defense)
            .unwrap_or(0);
    format!(
        "S: EVT CHAR_STATS {} {} {} {} {} {}\n",
        total_dmg, total_spe_dmg, total_def, total_spe_def, player.max_hp, player.stat_points
    )
}

pub(crate) fn player_stats_event(player: &crate::state::Player) -> String {
    format!(
        "S: EVT PLAYER_STATS {} {} {} {} {}\n",
        player.hp, player.max_hp, player.xp_bar.current, player.xp_bar.requiered, player.level
    )
}
