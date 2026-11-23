//! NPC interaction: `INTERACT`, `TALK` and `ANSWER`.

use std::net::SocketAddr;
use serde_json::json;
use tokio::sync::broadcast::Sender;

use domain::quest::QuestStage;
use domain::{CombatState, ItemId};

use crate::state::ServerState;
use crate::world::WorldData;
use crate::events::{to_player, GlobalEvent};
use crate::handlers::board::handle_board_quests;
use crate::handlers::combat::{handle_attack, start_combat};
use crate::handlers::player_events::{get_char_stats_event_with_world, get_equip_data_event, get_inv_data_event};
use crate::handlers::quest_core::{advance_quest_stage, complete_staged_quest, start_quest};
use crate::handlers::quest_talk::resolve_talk_quests;
use crate::handlers::shop::shop_event;
use crate::handlers::util::ci_eq;

/// Selects the most relevant conditional dialogue block for an NPC
/// based on the current stage of the first active quest linked to this NPC.
pub(crate) fn pick_quest_dialogue<'a>(
    npc: &'a crate::world::Npc,
    player: &crate::state::Player,
    world: &WorldData,
    just_completed: &[domain::QuestId],
) -> Option<&'a crate::world::QuestDialogue> {
    // Stage 254 = "just completed": shown once, in the conversation that completes the quest.
    for qid in just_completed {
        if let Some(qd) = npc
            .quest_dialogues
            .iter()
            .find(|qd| &qd.quest_id == qid && qd.stage == 254) {
            return Some(qd);
        }
    }

    // Blocks are tried in the order of the quests in world.yaml; the first quest that has a
    // block for the player's current state wins.
    for q in &world.world.quests {
        // The NPC takes part in a quest if it gives it, is its target, or declares dialogue
        // blocks for it (e.g. an NPC whose text changes with the main story line).
        let is_giver = q.giver_id.as_ref() == Some(&npc.id);
        let is_target = is_npc_quest_target(npc, q);
        let has_blocks = npc.quest_dialogues.iter().any(|qd| qd.quest_id == q.id);
        if !is_giver && !is_target && !has_blocks {
            continue;
        }

        if player.completed_quests.contains(&q.id) {
            if q.giver_id.as_ref() == Some(&npc.id)
                && player.inventory.contains(&ItemId::from("dragon_eye")) {
                let maybe = npc
                    .quest_dialogues
                    .iter()
                    .find(|qd| qd.quest_id == q.id && qd.stage == 256);
                if maybe.is_some() {
                    return maybe;
                }
            }

            // stage 255 = already completed
            let maybe = npc
                .quest_dialogues
                .iter()
                .find(|qd| qd.quest_id == q.id && qd.stage == 255);
            if maybe.is_some() {
                return maybe;
            }
            continue;
        }

        // 0 = not started, 1..N = Stage(n)
        let current_stage_num = player
            .quest_progress
            .iter()
            .find(|p| p.quest_id == q.id)
            .map(|p| match &p.stage {
                QuestStage::NotStarted => 0,
                QuestStage::Stage(n) => *n,
                QuestStage::Completed => 255,
            })
            .unwrap_or(0);

        let maybe = npc
            .quest_dialogues
            .iter()
            .find(|qd| qd.quest_id == q.id && qd.stage == current_stage_num);
        if maybe.is_some() {
            return maybe;
        }
    }
    None
}

pub(crate) fn is_npc_quest_target(npc: &crate::world::Npc, q: &crate::world::Quest) -> bool {
    use crate::world::QuestObjective;
    match &q.objective {
        QuestObjective::TalkToNpc { npc: n } => n == &npc.id,
        QuestObjective::DeliverItem { to, .. } => to == &npc.id,
        QuestObjective::DeliverItems { to, .. } => to == &npc.id,
        _ => false,
    }
}

pub(crate) fn handle_talk(
    addr: SocketAddr,
    cible: String,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    // ── 1. Find the NPC ─────────────────────────────────────────────────
    let current_room_id;
    let npc_trouve: Option<crate::world::Npc>;
    {
        let Some(player) = state.players.get(&addr) else {
            return "S: ERR utilize_connect_first\n".to_string();
        };
        current_room_id = player.current_room.clone();
        npc_trouve = state.room_npcs.get(&current_room_id).and_then(|ids| {
            ids.iter().find_map(|npc_id| {
                world
                    .world
                    .npcs
                    .iter()
                    .find(|n| {
                        &n.id == npc_id && (ci_eq(n.id.as_str(), &cible) || ci_eq(&n.name, &cible))
                    })
                    .cloned()
            })
        });
    }

    let Some(npc) = npc_trouve else {
        return "S: ERR There is nobody by that name here.\n".to_string();
    };

    let Some(player) = state.players.get(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };

    // ── 2. Quest resolution (legacy: DeliverItem / TalkToNpc) ──────────
    if npc.id == domain::NpcId::from("Temple_guard") {
        let has_pass = player.inventory.contains(&ItemId::from("temple_pass"));
        let lines = if has_pass {
            vec!["The queen's pass opens the way. You may proceed.".to_string()]
        } else {
            vec!["The temple is sealed. Present the queen's pass to enter.".to_string()]
        };
        to_player(
            tx,
            format!("S: EVT DIALOGUE {} {}\n", npc.name.replace(' ', "_"), lines.join("|")),
            addr,
        );
        return format!("S: OK npc={}\n", npc.name);
    }

    let Some((quest_msgs, completed_now)) = resolve_talk_quests(addr, &npc, state, world, tx)
    else {
        return "S: ERR utilize_connect_first\n".to_string();
    };

    // ── 3. Pick the right dialogue (conditional or default) ─────────────
    //    The text is chosen BEFORE this NPC starts its quests, so the "not started"
    //    block (stage 0) is what the player sees on a first meeting.
    let Some(player) = state.players.get(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    let npc_key = npc.name.replace(' ', "_");

    let dialogue_text: String;
    if let Some(qd) = pick_quest_dialogue(&npc, player, world, &completed_now) {
        // The lines of the block, then the reward summary if a quest was just completed.
        let mut lines: Vec<String> = qd.lines.clone();
        lines.extend(quest_msgs.iter().cloned());
        dialogue_text = lines.join(" ");
        let pages = lines.join("|");
        if qd.choices.is_empty() {
            to_player(
                tx,
                format!("S: EVT DIALOGUE {} {}\n", npc_key, pages),
                addr,
            );
        } else {
            // Choice dialogue: `S: EVT DIALOGUE_CHOICE <npc_key> <pages>||CHOICES||<text>:<answer>|...`
            let choices_str = qd
                .choices
                .iter()
                .map(|c| format!("{}:{}", c.text.replace(' ', "_"), c.answer))
                .collect::<Vec<_>>()
                .join("|");
            to_player(
                tx,
                format!(
                    "S: EVT DIALOGUE_CHOICE {} {}||CHOICES||{}\n",
                    npc_key, pages, choices_str
                ),
                addr,
            );
        }
    } else if !quest_msgs.is_empty() {
        // A quest was completed but this NPC has no dedicated text: show only the reward.
        dialogue_text = quest_msgs.join(" ");
        to_player(
            tx,
            format!("S: EVT DIALOGUE {} {}\n", npc_key, quest_msgs.join("|")),
            addr,
        );
    } else {
        dialogue_text = npc.dialogue.join(" ");
        to_player(
            tx,
            format!("S: EVT DIALOGUE {} {}\n", npc_key, npc.dialogue.join("|")),
            addr,
        );
    }

    // ── 4. Start the quests this NPC gives (if not started / not completed) ──
    {
        let Some(player) = state.players.get_mut(&addr) else {
            return "S: ERR utilize_connect_first\n".to_string();
        };
        for q in &world.world.quests {
            if q.giver_id.as_ref() == Some(&npc.id)
                && !player.completed_quests.contains(&q.id)
                && !player.quest_progress.iter().any(|p| p.quest_id == q.id) {
                start_quest(addr, q, player, tx);
            }
        }
    }

    // Rewards (XP, items, equipment) changed: refresh the client's stats, inventory and equipment panels.
    if !quest_msgs.is_empty() {
        if let Some(player) = state.players.get(&addr) {
            to_player(
                tx,
                format!(
                    "S: EVT PLAYER_STATS {} {} {} {} {}\n{}{}{}",
                    player.hp,
                    player.max_hp,
                    player.xp_bar.current,
                    player.xp_bar.requiered,
                    player.level,
                    get_inv_data_event(player, world),
                    get_equip_data_event(player, world),
                    get_char_stats_event_with_world(player, world)
                ),
                addr,
            );
        }
    }

    // RFC 42TAP: OK { "npc": "guard", "dialogue": "Stay safe, traveler." }
    // (the paged dialogue itself is pushed to the client with EVT DIALOGUE / DIALOGUE_CHOICE)

    // ── 5. If NPC is a merchant, push the shop listing ───────────────────
    if !npc.shop.is_empty() {
        if let Some(player) = state.players.get(&addr) {
            to_player(
                tx,
                shop_event(npc.id.as_str(), player.money, &npc, player, world),
                addr,
            );
        }
    }

    format!(
        "S: OK {}\n",
        json!({ "npc": npc.id.as_str(), "dialogue": dialogue_text })
    )
}

/// Handles the player's answer to a multiple-choice dialogue.
pub(crate) fn handle_answer(
    addr: SocketAddr,
    npc_id: String,
    answer: String,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    let Some(player) = state.players.get(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };

    // Find the quest expecting an answer at this stage
    let matching_quest: Option<(
        domain::QuestId,
        bool,
        String,
        Option<crate::world::OnWrongAnswer>,
    )> = world.world.quests.iter().find_map(|q| {
        if player.completed_quests.contains(&q.id) {
            return None;
        }

        let current_stage = player
            .quest_progress
            .iter()
            .find(|p| p.quest_id == q.id)?
            .stage
            .clone();
        let stage_num = current_stage.stage_num()?;
        let stage_def = q.stages.iter().find(|s| s.stage == stage_num)?;

        if let Some(domain::quest::StageObjective::AnswerDialogue {
            npc,
            expected_answer,
        }) = &stage_def.objective {
            if ci_eq(npc.as_str(), &npc_id) {
                let is_last = q.stages.iter().all(|s| s.stage <= stage_num);
                return Some((
                    q.id.clone(),
                    is_last,
                    expected_answer.clone(),
                    stage_def.on_wrong_answer.clone(),
                ));
            }
        }
        None
    });

    match matching_quest {
        None => "S: ERR no_pending_dialogue_answer\n".to_string(),
        Some((quest_id, is_last, expected, wrong_answer_cfg)) => {
            if !ci_eq(&answer, &expected) {
                // Wrong answer: apply on_wrong_answer behaviour if defined.
                let mut out = String::new();
                if let Some(cfg) = wrong_answer_cfg {
                    if cfg.reset {
                        // Remove progress → player can restart.
                        if let Some(player) = state.players.get_mut(&addr) {
                            player.quest_progress.retain(|p| p.quest_id != quest_id);
                        } else {
                            return "S: ERR utilize_connect_first\n".to_string();
                        }
                        to_player(
                            tx,
                            format!("S: EVT QUEST_RESET {}\n", quest_id),
                            addr,
                        );
                    }
                    if !cfg.dialogue.is_empty() {
                        let pages = cfg.dialogue.join("|");
                        to_player(
                            tx,
                            format!("S: EVT DIALOGUE {} {}\n", npc_id, pages),
                            addr,
                        );
                    }
                }
                out.push_str("S: OK wrong_answer\n");
                return out;
            }

            // Correct answer
            let Some(player) = state.players.get_mut(&addr) else {
                return "S: ERR utilize_connect_first\n".to_string();
            };
            if is_last {
                let Some(q) = world
                    .world
                    .quests
                    .iter()
                    .find(|q| q.id == quest_id)
                    .cloned()
                else {
                    return "S: ERR unknown_quest\n".to_string();
                };
                let reward_msg = complete_staged_quest(addr, &q, player, world, tx);
                format!("S: OK correct_answer {}\n", reward_msg)
            } else {
                advance_quest_stage(addr, &quest_id, player, tx);
                "S: OK correct_answer\n".to_string()
            }
        }
    }
}

pub(crate) fn handle_interact(
    addr: SocketAddr,
    cible: String,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    let mut role = String::new();
    let mut found_npc = None;
    if let Some(player) = state.players.get(&addr) {
        let current_room_id = player.current_room.clone();
        if let Some(npcs_ici) = state.room_npcs.get(&current_room_id) {
            for npc_id in npcs_ici {
                if let Some(npc) = world.npc_by_id(npc_id) {
                    if ci_eq(npc.id.as_str(), &cible) || ci_eq(&npc.name, &cible) {
                        role = npc.role.clone();
                        found_npc = Some(npc.clone());
                        break;
                    }
                }
            }
        }
    }

    if role == "enemy" {
        if let Some(npc) = found_npc {
            let in_combat = state.players.get(&addr).is_some_and(|p| match &p.combat {
                CombatState::InCombat { target } => target == &npc.id,
                _ => false,
            });
            if in_combat {
                handle_attack(addr, cible, state, world, tx)
            } else {
                start_combat(addr, &npc, state)
            }
        } else {
            "S: ERR There is nobody by that name here.\n".to_string()
        }
    } else if !role.is_empty() {
        // Check if this NPC is a board ("Panneau") → return board quest list.
        if role == "board"
            || (found_npc.as_ref().is_some_and(|n| {
                ci_eq(n.id.as_str(), "Panneau") || ci_eq(n.id.as_str(), "panneau")
            })) {
            handle_board_quests(addr, state)
        } else {
            handle_talk(addr, cible, state, world, tx)
        }
    } else {
        "S: ERR There is nothing to interact with here.\n".to_string()
    }
}
