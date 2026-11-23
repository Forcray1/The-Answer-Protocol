//! Combat core: start, flee, stunned turns and the physical `ATTACK`.

use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::Sender;
use rand::Rng;

use domain::{CombatState, RoomId};

use crate::state::{CombatInstance, CombatPhase, ServerState, START_ROOM};
use crate::world::{Npc, WorldData};
use crate::events::{to_room, GlobalEvent};
use crate::handlers::outcome::{announce_victory, kill_player, remove_dead_monster, reward_kill};
use crate::handlers::player_events::{player_physical_attack_total, player_stats_event};
use crate::handlers::util::ci_eq;
use crate::handlers::wire::room_change_snapshot;

pub(crate) fn get_combat_bg_for_room(room: &RoomId) -> u32 {
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

pub fn handle_flee(addr: SocketAddr, state: &mut ServerState, tx: &Sender<GlobalEvent>) -> String {
    let Some(player) = state.players.get(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };

    let room = player.current_room.clone();
    let username = player.username.clone();
    let success = rand::thread_rng().gen_range(0..100) < 75;

    if success {
        if let Some(player) = state.players.get_mut(&addr) {
            player.combat = CombatState::Idle;
        }
        state.active_combats.remove(&addr);
        to_room(
            tx,
            Some(addr),
            format!("S: EVT GLOBAL CHAT {} fled from combat and escaped.\n", username),
            room.clone(),
        );
        return "S: EVT COMBAT END flee\nS: OK You fled from combat.\n".to_string();
    }

    "S: EVT COMBAT END flee_failed\nS: OK Your escape attempt failed. You remain in combat.\n".to_string()
}

pub(crate) fn start_combat(addr: SocketAddr, npc: &Npc, state: &mut ServerState) -> String {
    let Some(player) = state.players.get_mut(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };

    let room = player.current_room.clone();
    player.combat = CombatState::InCombat {
        target: npc.id.clone(),
    };
    state.active_combats.insert(
        addr,
        CombatInstance {
            npc_id: npc.id.clone(),
            phase: CombatPhase::WaitingForPlayerAction,
            turn_number: 1,
        },
    );

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

pub(crate) fn damage_after_defense(base_damage: i32, defense_points: i32) -> i32 {
    (base_damage - defense_points).max(1)
}

pub(crate) fn handle_stunned_turn(
    addr: SocketAddr,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> Option<String> {
    let (player_name, target_npc_id, player_armor) = {
        let player = state.players.get(&addr)?;
        if player.stunned_turns == 0 {
            return None;
        }
        let target = match &player.combat {
            CombatState::InCombat { target } => target.clone(),
            _ => return None,
        };
        (
            player.username.clone(),
            target,
            player.stats.defense + player.equipement.total_defense(),
        )
    };

    let npc = world.npc_by_id(&target_npc_id)?;
    let monster_name = npc.name.clone();
    let max_hp_monster = npc.hp;
    let monster_hp_left = state
        .npc_hps
        .get(&target_npc_id)
        .copied()
        .unwrap_or(max_hp_monster);
    let monster_damage = npc.damage.unwrap_or(5);
    let final_monster_damage = damage_after_defense(monster_damage, player_armor);

    let (player_dead, player_hp, max_hp) = if let Some(player) = state.players.get_mut(&addr) {
        if player.stunned_turns > 0 {
            player.stunned_turns -= 1;
        }
        player.hp -= final_monster_damage;
        if player.hp <= 0 {
            (true, player.hp, player.max_hp)
        } else {
            (false, player.hp, player.max_hp)
        }
    } else {
        return None;
    };

    let stat_str = if let Some(player) = state.players.get(&addr) {
        player_stats_event(player)
    } else {
        String::new()
    };

    if player_dead {
        let final_stats = kill_player(addr, &player_name, &monster_name, state, tx);
        Some(format!(
            "S: EVT COMBAT STUN {}\nS: EVT COMBAT END defeat\nS: OK {} is stunned by the last attack and can't move. {} attacks and finishes them off. DEAD! You wake up at the oasis.\n{}\n{}",
            player_name.replace(' ', "_"),
            player_name,
            monster_name,
            room_change_snapshot(addr, &RoomId::from(START_ROOM), state, world),
            final_stats
        ))
    } else {
        Some(format!(
            "S: EVT COMBAT STUN {}\nS: EVT COMBAT STUN_HIT {} {} {}\nS: OK {} is stun by the last attack and can't move. {} attacks (-{} HP). (HP: {}/{})\n{}",
            player_name.replace(' ', "_"),
            monster_hp_left,
            max_hp_monster,
            final_monster_damage,
            player_name,
            monster_name,
            final_monster_damage,
            player_hp,
            max_hp,
            stat_str
        ))
    }
}

/// Extra damage of a heavy attack: 75% of the attack stat after defense.
pub(crate) fn heavy_bonus_damage(attack_total: i32, defense: i32) -> i32 {
    (((attack_total - defense).max(1) as f32 * 0.75).round() as i32).max(1)
}

pub(crate) fn handle_attack(
    addr: SocketAddr,
    cible: String,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    if let Some(stunned_response) = handle_stunned_turn(addr, state, world, tx) {
        return stunned_response;
    }
    resolve_attack(addr, cible, false, state, world, tx)
}

/// Resolves one physical hit. A heavy hit includes its bonus before the death check,
/// so a lethal blow ends the fight without any retaliation.
pub(crate) fn resolve_attack(
    addr: SocketAddr,
    cible: String,
    heavy: bool,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    let current_room_id;
    let attack_total;
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
        if player
            .last_attack
            .is_some_and(|last| last.elapsed() < Duration::from_millis(1000)) {
            on_cooldown = true;
        }
    }
    if on_cooldown {
        return "S: ERR attack_cooldown\n".to_string();
    }

    if let Some(player) = state.players.get_mut(&addr) {
        current_room_id = player.current_room.clone();
        player_name = player.username.clone();
        player.last_attack = Some(Instant::now());
        attack_total = player_physical_attack_total(player, world);
        player_damage += attack_total;
        player_armor += player.stats.defense + player.equipement.total_defense();
    } else {
        return "S: ERR utilize_connect_first\n".to_string();
    }

    if let Some(npcs_in_room) = state.room_npcs.get(&current_room_id) {
        for npc_id in npcs_in_room {
            if let Some(npc) = world.npc_by_id(npc_id) {
                if npc.role == "enemy"
                    && (ci_eq(npc.id.as_str(), &cible) || ci_eq(&npc.name, &cible)) {
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
        let mut final_damage = damage_after_defense(player_damage, monster_defense);
        if heavy {
            final_damage += heavy_bonus_damage(attack_total, monster_defense);
        }
        if let Some(hp) = state.npc_hps.get_mut(&m_id) {
            *hp -= final_damage;
            monster_hp_left = *hp;
            if *hp <= 0 {
                monster_dead = true;
            }
        }
        if monster_dead {
            remove_dead_monster(addr, &m_id, &current_room_id, state, world);
            let (drop_names, stat_str) =
                reward_kill(addr, &m_id, exp_gained, monster_drops, state, world, tx);
            announce_victory(
                addr,
                &player_name,
                &m_id,
                &current_room_id,
                format!("You dealt {} damage", final_damage),
                &monster_name,
                exp_gained,
                &drop_names,
                stat_str,
                state,
                world,
                tx,
            )
        } else {
            let mut stat_str = String::new();
            let mut player_dead = false;
            let mut player_hp = 0;
            let final_monster_damage = damage_after_defense(degats_monstre, player_armor);
            if let Some(player) = state.players.get_mut(&addr) {
                player.hp -= final_monster_damage;
                player_hp = player.hp;
                if player.hp <= 0 {
                    player_dead = true;
                }
                stat_str = player_stats_event(player);
            }
            if player_dead {
                let final_stats = kill_player(addr, &player_name, &monster_name, state, tx);
                format!(
                    "S: EVT COMBAT END defeat\nS: OK You deal {} damage, but {} finishes you off. You are DEAD! You wake up at the oasis.\n{}\n{}",
                    final_damage,
                    monster_name,
                    room_change_snapshot(addr, &RoomId::from(START_ROOM), state, world),
                    final_stats
                )
            } else {
                format!("S: EVT COMBAT HIT {} {} {} {}\nS: OK You attack {} ({} HP left). It retaliates (-{} HP). (Your HP: {})\n{}", monster_hp_left, max_hp_monster, final_damage, final_monster_damage, monster_name, monster_hp_left, final_monster_damage, player_hp, stat_str)
            }
        }
    } else {
        "S: ERR You can't attack that.\n".to_string()
    }
}
