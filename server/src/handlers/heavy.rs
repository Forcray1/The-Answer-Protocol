//! Heavy attacks: a single hit worth the base damage plus a 75% bonus, then a stun.

use std::net::SocketAddr;
use tokio::sync::broadcast::Sender;

use domain::CombatState;

use crate::state::ServerState;
use crate::world::WorldData;
use crate::events::GlobalEvent;
use crate::handlers::combat::{handle_stunned_turn, resolve_attack};
use crate::handlers::magic::resolve_magic_attack;

/// Heavy attack: 1.75× the damage of the appropriate attack type.
pub(crate) fn handle_attack_heavy(
    addr: SocketAddr,
    cible: String,
    magic: bool,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    if let Some(stunned_response) = handle_stunned_turn(addr, state, world, tx) {
        return stunned_response;
    }
    let response = if magic {
        resolve_magic_attack(addr, cible, true, state, world, tx)
    } else {
        resolve_attack(addr, cible, true, state, world, tx)
    };
    if response.starts_with("S: ERR") {
        return response;
    }
    // The stun only applies when the fight goes on.
    match state.players.get_mut(&addr) {
        Some(player) if matches!(player.combat, CombatState::InCombat { .. }) => {
            player.stunned_turns = 1;
            format!("{}S: EVT COMBAT STUN_APPLIED 1\n", response)
        }
        _ => response,
    }
}
