//! `EQUIP` / `UNEQUIP` and the conversion catalog item -> equipped weapon/armor.

use std::collections::HashMap;
use std::net::SocketAddr;

use domain::{Armor, Equipement, ItemBucket, ItemId, Weapon, WeaponType};

use crate::state::ServerState;
use crate::world::{Item as CatalogItem, WorldData};
use crate::handlers::player_events::{get_char_stats_event_with_world, get_equip_data_event, get_inv_data_event};
use crate::handlers::util::ci_eq;

pub(crate) fn make_weapon(item: &CatalogItem) -> Weapon {
    Weapon {
        id: item.id.clone(),
        name: item.name.clone(),
        damages: item.damage.unwrap_or(0),
        category: WeaponType::Melee,
        sprite: item.sprite.clone(),
    }
}

/// Builds a domain `Armor` from a catalog item.
pub(crate) fn make_armor(item: &CatalogItem) -> Armor {
    Armor {
        id: item.id.clone(),
        name: item.name.clone(),
        description: item.description.clone(),
        defense: item.defense.or(item.damage).unwrap_or(0),
        special_defense: 0,
        sprite: item.sprite.clone(),
    }
}

/// Rebuilds structured equipment from the slot -> id table (DB),
/// re-reading stats from the catalog.
pub(crate) fn build_equipement(world: &WorldData, equip: &HashMap<String, ItemId>) -> Equipement {
    let mut equipement = Equipement::default();
    for (slot, item_id) in equip {
        if let Some(item) = world.item_by_id(item_id) {
            if slot == "weapon" {
                equipement.weapon = Some(make_weapon(item));
            } else if let Some(slot_ref) = equipement.armor_slot_mut(slot) {
                *slot_ref = Some(make_armor(item));
            }
        }
    }
    equipement
}

pub(crate) fn handle_equip(
    addr: SocketAddr,
    cible: String,
    state: &mut ServerState,
    world: &WorldData,
) -> String {
    let Some(player) = state.players.get_mut(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    let Some(item) = world.find_item(&cible) else {
        return "S: ERR Unknown item.\n".to_string();
    };
    if !player.inventory.contains(&item.id) {
        return "S: ERR You don't have this item.\n".to_string();
    }
    let slot = match &item.slot {
        Some(s) => s.clone(),
        None => return "S: ERR This item cannot be equipped.\n".to_string(),
    };

    if slot == "medaillon" {
        // The medaillon leaves the inventory; a previously equipped one goes back to it.
        let old_id = player.equipped_medaillon_id.replace(item.id.clone());
        player.inventory.remove_one(&item.id);
        let msg = if let Some(old) = old_id {
            player.inventory.add(old, ItemBucket::Item);
            format!("S: OK Swapped medaillon for {}.\n", item.name)
        } else {
            format!("S: OK You equipped {} in slot [medaillon].\n", item.name)
        };
        return format!(
            "{}{}{}{}",
            msg,
            get_inv_data_event(player, world),
            get_equip_data_event(player, world),
            get_char_stats_event_with_world(player, world)
        );
    }

    if slot == "weapon" {
        let old = player.equipement.weapon.take();
        player.equipement.weapon = Some(make_weapon(item));
        if let Some(old) = old {
            player.inventory.add(old.id, ItemBucket::Weapon);
        }
        player.inventory.remove_one(&item.id);
        format!(
            "S: OK You equipped {} in slot [{}].\n{}{}{}",
            item.name,
            slot,
            get_inv_data_event(player, world),
            get_equip_data_event(player, world),
            get_char_stats_event_with_world(player, world)
        )
    } else if let Some(slot_ref) = player.equipement.armor_slot_mut(&slot) {
        let old = slot_ref.take();
        *slot_ref = Some(make_armor(item));
        if let Some(old) = old {
            player.inventory.add(old.id, ItemBucket::Item);
        }
        player.inventory.remove_one(&item.id);
        format!(
            "S: OK You equipped {} in slot [{}].\n{}{}{}",
            item.name,
            slot,
            get_inv_data_event(player, world),
            get_equip_data_event(player, world),
            get_char_stats_event_with_world(player, world)
        )
    } else {
        "S: ERR This item cannot be equipped.\n".to_string()
    }
}

pub(crate) fn handle_unequip(
    addr: SocketAddr,
    cible: String,
    state: &mut ServerState,
    world: &WorldData,
) -> String {
    let Some(player) = state.players.get_mut(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    // Medaillon unequip: it goes back to the inventory.
    if let Some(mid) = player.equipped_medaillon_id.clone() {
        if ci_eq(mid.as_str(), &cible) {
            player.equipped_medaillon_id = None;
            player.inventory.add(mid, ItemBucket::Item);
            return format!(
                "S: OK You unequipped the medaillon.\n{}{}{}",
                get_inv_data_event(player, world),
                get_equip_data_event(player, world),
                get_char_stats_event_with_world(player, world)
            );
        }
    }

    let found = player
        .equipement
        .equipped()
        .into_iter()
        .find(|(_, id, name)| ci_eq(id.as_str(), &cible) || ci_eq(name, &cible));
    let Some((slot, id, name)) = found else {
        return "S: ERR You are not wearing this item.\n".to_string();
    };
    if slot == "weapon" {
        player.equipement.weapon = None;
        player.inventory.add(id, ItemBucket::Weapon);
    } else if let Some(slot_ref) = player.equipement.armor_slot_mut(slot) {
        *slot_ref = None;
        player.inventory.add(id, ItemBucket::Item);
    }
    format!(
        "S: OK You unequipped {}.\n{}{}{}",
        name,
        get_inv_data_event(player, world),
        get_equip_data_event(player, world),
        get_char_stats_event_with_world(player, world)
    )
}
