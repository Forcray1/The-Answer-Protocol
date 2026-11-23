//! Quest board: `BOARD_QUESTS`, `BOARD_TAKE`, `BOARD_ABANDON` and kill tracking.

use std::net::SocketAddr;

use crate::state::ServerState;
use crate::world::WorldData;
use crate::logging::log_event;
use crate::handlers::quest_log::quest_data_event;

/// `BOARD_QUESTS` — return the 6 board quest slots to the requesting player.
pub(crate) fn handle_board_quests(addr: SocketAddr, state: &ServerState) -> String {
    if !state.players.contains_key(&addr) {
        return "S: ERR utilize_connect_first\n".to_string();
    }
    let wire = state.board_quests.to_wire();
    format!("S: EVT BOARD_QUESTS {}\n", wire)
}

/// `BOARD_TAKE <slot>` — player accepts the quest in the given 1-based slot.
pub(crate) fn handle_board_take(
    addr: SocketAddr,
    slot: usize,
    state: &mut ServerState,
    world: &WorldData,
) -> String {
    let player_name = match state.players.get(&addr) {
        Some(p) => p.username.clone(),
        None => return "S: ERR utilize_connect_first\n".to_string(),
    };
    // Player may only hold one board quest at a time.
    if state.board_quests.player_slot(&player_name).is_some() {
        return "S: ERR You already have an active board quest. Abandon it first.\n".to_string();
    }
    let idx = slot.wrapping_sub(1); // convert 1-based to 0-based
    match state.board_quests.slots.get_mut(idx) {
        Some(s) if s.is_available() => {
            s.taken_by = Some(player_name.clone());
            s.kills_done = 0;
            let mob = s.mob_name.clone();
            let count = s.kill_count;
            let loc = s.location_hint.clone();
            let gold = s.gold_reward;
            log_event(
                "BOARD_TAKE",
                &player_name,
                serde_json::json!({"slot": slot, "mob": mob, "count": count}),
            );
            let wire = state.board_quests.to_wire();
            // Send the full journal so the other active quests stay listed.
            let quest_data = state
                .players
                .get(&addr)
                .map(|p| quest_data_event(p, state, world))
                .unwrap_or_default();
            format!(
                "S: OK Board quest accepted: kill {} {} {} ({} gold).\nS: EVT BOARD_QUESTS {}\n{}",
                count, mob, loc, gold, wire, quest_data
            )
        }
        Some(_) => "S: ERR This quest is already taken by another player.\n".to_string(),
        None => "S: ERR Invalid slot number (1-6).\n".to_string(),
    }
}

/// `BOARD_ABANDON` — player abandons their current board quest; slot becomes available again.
pub(crate) fn handle_board_abandon(
    addr: SocketAddr,
    state: &mut ServerState,
    world: &WorldData,
) -> String {
    let player_name = match state.players.get(&addr) {
        Some(p) => p.username.clone(),
        None => return "S: ERR utilize_connect_first\n".to_string(),
    };
    if let Some(slot) = state.board_quests.player_slot_mut(&player_name) {
        slot.taken_by = None;
        slot.kills_done = 0;
        let wire = state.board_quests.to_wire();
        log_event("BOARD_ABANDON", &player_name, serde_json::json!({}));
        // Send the full journal: only the board quest disappears from it.
        let quest_data = state
            .players
            .get(&addr)
            .map(|p| quest_data_event(p, state, world))
            .unwrap_or_default();
        format!(
            "S: OK Board quest abandoned.\nS: EVT BOARD_QUESTS {}\n{}",
            wire, quest_data
        )
    } else {
        "S: ERR You have no active board quest.\n".to_string()
    }
}

/// Called whenever a player kills a mob. Updates board quest progress and,
/// if the quest is complete, gives the gold reward and regenerates the slot.
/// Returns an extra message to append to the attack result (empty if not applicable).
pub fn process_board_quest_kill(
    player_name: &str,
    mob_id: &str,
    state: &mut ServerState,
) -> String {
    let mut extra = String::new();
    let mut completed_slot_idx: Option<usize> = None;
    let mut gold_reward = 0i32;

    if let Some(slot) = state.board_quests.player_slot_mut(player_name) {
        if slot.matches_mob(mob_id) {
            slot.kills_done += 1;
            if slot.kills_done >= slot.kill_count {
                completed_slot_idx = Some(slot.slot_id);
                gold_reward = slot.gold_reward;
            } else {
                extra = format!(
                    " [BOARD QUEST] {} {}/{}",
                    slot.mob_name, slot.kills_done, slot.kill_count
                );
            }
        }
    }

    if let Some(idx) = completed_slot_idx {
        // Give gold to player.
        if let Some(player) = state
            .players
            .values_mut()
            .find(|p| p.username == player_name) {
            player.add_money(gold_reward);
            extra = format!(" [BOARD QUEST COMPLETE] +{} gold!", gold_reward);
        }
        // Regenerate the slot.
        state.board_quests.slots[idx] = crate::board_quests::BoardQuestSlot::generate(idx);
        log_event(
            "BOARD_COMPLETE",
            player_name,
            serde_json::json!({"gold": gold_reward}),
        );
    }

    extra
}
