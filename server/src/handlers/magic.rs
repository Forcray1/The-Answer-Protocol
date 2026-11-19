use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::Sender;

use domain::RoomId;

use crate::state::{ServerState, START_ROOM};
use crate::world::WorldData;
use crate::events::GlobalEvent;
use crate::handlers::combat::{damage_after_defense, handle_stunned_turn};
use crate::handlers::outcome::{announce_victory, kill_player, remove_dead_monster, roll_drops};
use crate::handlers::player_events::{player_special_attack_total, player_stats_event};
use crate::handlers::util::ci_eq;
use crate::handlers::wire::room_change_snapshot;

pub(crate) fn handle_attack_magic(
    addr: SocketAddr,
    cible: String,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    if let Some(stunned_response) = handle_stunned_turn(addr, state, world, tx) {
        return stunned_response;
    }
    // Same as handle_attack but scales on special_damage / hits special_defense
    let current_room_id;
    let mut player_spe_dmg = 5;
    let mut player_armor = 0;
    let mut monster_id_found = None;
    let mut monster_name = String::new();
    let player_name;
    let mut monster_damage = 5;
    let mut monster_spe_def = 0;
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
        player_spe_dmg += player_special_attack_total(player, world);
        player_armor += player.stats.special_defense + player.equipement.total_defense();
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
                    monster_damage = npc.damage.unwrap_or(5);
                    monster_spe_def = npc.special_defense.unwrap_or(0);
                    exp_gained = npc.exp_reward.unwrap_or(0);
                    monster_drops = npc.drops.clone();
                    max_hp_monster = npc.hp;
                    break;
                }
            }
        }
    }

    if let Some(m_id) = monster_id_found {
        let final_damage = (player_spe_dmg - monster_spe_def).max(1);
        let mut monster_dead = false;
        let mut monster_hp_left = 0;
        if let Some(hp) = state.npc_hps.get_mut(&m_id) {
            *hp -= final_damage;
            monster_hp_left = *hp;
            if *hp <= 0 {
                monster_dead = true;
            }
        }
        if monster_dead {
            remove_dead_monster(addr, &m_id, &current_room_id, state, world);
            let mut drop_names = Vec::new();
            let mut stat_str = String::new();
            if let Some(player) = state.players.get_mut(&addr) {
                player.add_xp_with_world(exp_gained, world);
                drop_names.extend(roll_drops(player, monster_drops, world));
                stat_str = player_stats_event(player);
            }
            announce_victory(
                addr,
                &player_name,
                &m_id,
                &current_room_id,
                format!("Magic deals {}", final_damage),
                &monster_name,
                exp_gained,
                &drop_names,
                stat_str,
                state,
                tx,
            )
        } else {
            let mut stat_str = String::new();
            let mut player_dead = false;
            let mut player_hp = 0;
            let final_monster_damage = damage_after_defense(monster_damage, player_armor);
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
                    "S: EVT COMBAT END defeat\nS: OK Magic deals {}. {} retaliates and finishes you off. DEAD! You wake up at the oasis.\n{}\n{}",
                    final_damage,
                    monster_name,
                    room_change_snapshot(&RoomId::from(START_ROOM), state, world),
                    final_stats
                )
            } else {
                format!("S: EVT COMBAT HIT {} {} {} {}\nS: OK Magic deals {}. {} retaliates (-{} HP). (HP: {})\n{}", monster_hp_left, max_hp_monster, final_damage, final_monster_damage, final_damage, monster_name, final_monster_damage, player_hp, stat_str)
            }
        }
    } else {
        "S: ERR You can't attack that.\n".to_string()
    }
}
