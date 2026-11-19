use std::net::SocketAddr;
use tokio::sync::broadcast::Sender;

use domain::CombatState;

use crate::state::ServerState;
use crate::world::WorldData;
use crate::events::{to_player, GlobalEvent};
use crate::handlers::combat::handle_attack;
use crate::handlers::magic::handle_attack_magic;
use crate::handlers::player_events::{player_physical_attack_total, player_special_attack_total};
use crate::handlers::util::ci_eq;

/// Heavy attack: 1.75× the damage of the appropriate attack type.
pub(crate) fn handle_attack_heavy(
    addr: SocketAddr,
    cible: String,
    magic: bool,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    // Delegate to the correct base handler, then multiply final damage
    // We reuse handle_attack / handle_attack_magic but send ATTACK_HEAVY sentinel
    let base_resp = if magic {
        handle_attack_magic(addr, cible.clone(), state, world, tx)
    } else {
        handle_attack(addr, cible.clone(), state, world, tx)
    };
    if base_resp.starts_with("S: ERR") {
        return base_resp;
    }
    let apply_stun = state
        .players
        .get(&addr)
        .is_some_and(|p| matches!(p.combat, CombatState::InCombat { .. }));
    if apply_stun {
        if let Some(player) = state.players.get_mut(&addr) {
            player.stunned_turns = 1;
        }
    }
    // The base handler already dealt damage. We can't easily undo it.
    // Instead: call get_bonus_hit to deal extra 0.75× portion.
    apply_bonus_hit(addr, &cible, magic, state, world, tx);
    if apply_stun {
        format!("{}S: EVT COMBAT STUN_APPLIED 1\n", base_resp)
    } else {
        base_resp
    }
}

/// Applies the 75% bonus portion for a heavy attack (called after the base 100% hit).
pub(crate) fn apply_bonus_hit(
    addr: SocketAddr,
    cible: &str,
    magic: bool,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) {
    let current_room_id = match state.players.get(&addr) {
        Some(p) => p.current_room.clone(),
        None => return,
    };
    let monster_id = state.room_npcs.get(&current_room_id).and_then(|ids| {
        ids.iter().find_map(|npc_id| {
            let npc = world.npc_by_id(npc_id)?;
            if npc.role == "enemy" && (ci_eq(npc.id.as_str(), cible) || ci_eq(&npc.name, cible)) {
                Some(npc_id.clone())
            } else {
                None
            }
        })
    });
    let Some(m_id) = monster_id else { return };
    // Calculate the 75% bonus damage
    let bonus = if let Some(player) = state.players.get(&addr) {
        if magic {
            let atk = player_special_attack_total(player, world);
            let target_spe_def = world.npc_by_id(&m_id)
                .map(|n| n.special_defense.unwrap_or(0))
                .unwrap_or(0);
            let base_final = (atk - target_spe_def).max(1);
            ((base_final as f32) * 0.75).round() as i32
        } else {
            let atk = player_physical_attack_total(player, world);
            let target_def = world.npc_by_id(&m_id)
                .map(|n| n.defense.unwrap_or(0))
                .unwrap_or(0);
            let base_final = (atk - target_def).max(1);
            ((base_final as f32) * 0.75).round() as i32
        }
    } else {
        return;
    };

    let mut enemy_hp = 0;
    let mut enemy_max_hp = 1;
    if let Some(npc) = world.npc_by_id(&m_id) {
        enemy_max_hp = npc.hp;
    }
    if let Some(hp) = state.npc_hps.get_mut(&m_id) {
        *hp -= bonus.max(1);
        enemy_hp = *hp;
    }
    // The main hit already handles death/despawn in the same frame, so no need to recheck here.
    to_player(
        tx,
        format!(
            "S: EVT COMBAT BONUS_HIT {} {} {}\n",
            bonus.max(1),
            enemy_hp.max(0),
            enemy_max_hp
        ),
        addr,
    );
}
