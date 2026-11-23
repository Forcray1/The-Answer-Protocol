//! `BUY` and the merchant listing.

use std::net::SocketAddr;
use tokio::sync::broadcast::Sender;

use crate::state::{Player, ServerState};
use crate::world::{Npc, WorldData};
use crate::events::{to_player, GlobalEvent};
use crate::handlers::player_events::{get_inv_data_event, player_stats_event};
use crate::handlers::util::ci_eq;

pub(crate) fn handle_buy(
    addr: SocketAddr,
    npc_id: String,
    item_id: String,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    // ── 1. Find the NPC and its shop ────────────────────────────────────
    let npc = world
        .world
        .npcs
        .iter()
        .find(|n| ci_eq(n.id.as_str(), &npc_id) || ci_eq(&n.name, &npc_id));
    let npc = match npc {
        Some(n) => n.clone(),
        None => return "S: ERR npc_not_found\n".to_string(),
    };
    if npc.shop.is_empty() {
        return "S: ERR npc_has_no_shop\n".to_string();
    }
    let shop_entry = npc.shop.iter().find(|si| si.item_id.as_str() == item_id);
    let shop_entry = match shop_entry {
        Some(e) => e.clone(),
        None => return "S: ERR item_not_in_shop\n".to_string(),
    };
    let item_def = match world.world.items.iter().find(|i| i.id.as_str() == item_id) {
        Some(i) => i.clone(),
        None => return "S: ERR item_unknown\n".to_string(),
    };

    // ── 2. Check player state ────────────────────────────────────────────
    let Some(player) = state.players.get_mut(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    // One-per-player limit for non-consumables
    if !item_def.consumable {
        let already_equipped = player
            .equipement
            .equipped()
            .iter()
            .any(|(_, id, _)| id.as_str() == item_id);
        if player.inventory.contains(&shop_entry.item_id) || already_equipped {
            return "S: ERR already_owned\n".to_string();
        }
    }
    if player.money < shop_entry.price as i32 {
        return "S: ERR not_enough_gold\n".to_string();
    }

    // ── 3. Process the purchase ──────────────────────────────────────────
    player.money -= shop_entry.price as i32;
    let bucket = item_def.bucket();
    player.inventory.add(shop_entry.item_id.clone(), bucket);
    let gold_left = player.money;
    let item_name = item_def.name.clone();

    // Send inventory + stats refresh
    let inv_evt = get_inv_data_event(player, world);
    let stats_evt = player_stats_event(player);
    let gold_evt = format!("S: EVT GOLD {}\n", gold_left);

    // Refresh the shop listing so can_buy flags update
    let shop_evt = shop_event(&npc.id.to_string(), gold_left, &npc, player, world);

    to_player(
        tx,
        format!("{}{}{}{}", inv_evt, stats_evt, gold_evt, shop_evt),
        addr,
    );

    format!(
        "S: OK Bought {} for {} gold. ({} gold left)\n",
        item_name, shop_entry.price, gold_left
    )
}

/// `EVT SHOP` listing of a merchant: `item:name:price:sprite:can_buy` entries.
/// `can_buy` is 0 for a non-consumable the player already owns or wears.
pub(crate) fn shop_event(
    npc_key: &str,
    money: i32,
    npc: &Npc,
    player: &Player,
    world: &WorldData,
) -> String {
    let entries: Vec<String> = npc
        .shop
        .iter()
        .filter_map(|si| {
            let item_def = world.item_by_id(&si.item_id)?;
            let sprite = item_def
                .sprite
                .clone()
                .unwrap_or_else(|| "UI/QuestUI".to_string());
            let can_buy = if item_def.consumable {
                1
            } else {
                let already_equipped = player
                    .equipement
                    .equipped()
                    .iter()
                    .any(|(_, id, _)| id == &si.item_id);
                if player.inventory.contains(&si.item_id) || already_equipped {
                    0
                } else {
                    1
                }
            };
            Some(format!(
                "{}:{}:{}:{}:{}",
                si.item_id, item_def.name, si.price, sprite, can_buy
            ))
        })
        .collect();
    format!("S: EVT SHOP {} {} {}\n", npc_key, money, entries.join("|"))
}
