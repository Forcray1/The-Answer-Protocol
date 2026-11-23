//! Quest progress triggered by talking to an NPC.

use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::sync::broadcast::Sender;

use domain::quest::QuestStage;
use domain::ItemId;

use crate::state::ServerState;
use crate::world::{Npc, WorldData};
use crate::events::GlobalEvent;
use crate::handlers::quest_core::{advance_quest_stage, complete_legacy_quest, complete_staged_quest};

/// Completes or advances every quest that talking to `npc` resolves.
/// Returns the reward messages and the ids of the quests completed by this conversation
/// (`None` if the player is gone).
pub(crate) fn resolve_talk_quests(
    addr: SocketAddr,
    npc: &Npc,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> Option<(Vec<String>, Vec<domain::QuestId>)> {
    let player = state.players.get_mut(&addr)?;
    let mut quest_msgs: Vec<String> = Vec::new();
    // Quests completed during this very conversation (selects the "just completed" dialogue).
    let mut completed_now: Vec<domain::QuestId> = Vec::new();
    for q in &world.world.quests {
        if player.completed_quests.contains(&q.id) {
            continue;
        }

        // Multi-stage quests: check TalkToNpc at the current stage
        if !q.stages.is_empty() {
            let current_stage = player
                .quest_progress
                .iter()
                .find(|p| p.quest_id == q.id)
                .map(|p| p.stage.clone())
                .unwrap_or(QuestStage::NotStarted);

            if let Some(stage_num) = current_stage.stage_num() {
                if let Some(stage_def) = q.stages.iter().find(|s| s.stage == stage_num) {
                    match &stage_def.objective {
                        Some(domain::quest::StageObjective::TalkToNpc { npc: target })
                            if target == &npc.id => {
                            // Last stage → complete the quest
                            let is_last = q.stages.iter().all(|s| s.stage <= stage_num);
                            if is_last {
                                quest_msgs.push(complete_staged_quest(addr, q, player, world, tx));
                                completed_now.push(q.id.clone());
                            } else {
                                let qid = q.id.clone();
                                advance_quest_stage(addr, &qid, player, tx);
                            }
                        }
                        Some(domain::quest::StageObjective::FetchItem { item })
                            if q.giver_id.as_ref() == Some(&npc.id)
                                && player.inventory.contains(item) => {
                            // Remove item from inventory and advance
                            let item_id = item.clone();
                            player.inventory.remove_one(&item_id);
                            let is_last = q.stages.iter().all(|s| s.stage <= stage_num);
                            if is_last {
                                quest_msgs.push(complete_staged_quest(addr, q, player, world, tx));
                                completed_now.push(q.id.clone());
                            } else {
                                let qid = q.id.clone();
                                advance_quest_stage(addr, &qid, player, tx);
                            }
                        }
                        _ => {}
                    }
                }
            }
            continue;
        }

        // Legacy quests (single objective)
        match &q.objective {
            crate::world::QuestObjective::DeliverItem { item, to } => {
                if &npc.id == to && player.inventory.contains(item) {
                    player.inventory.remove_one(item);
                    quest_msgs.push(complete_legacy_quest(addr, q, player, world, tx));
                    completed_now.push(q.id.clone());
                }
            }
            crate::world::QuestObjective::DeliverItems { items, to } => {
                if &npc.id != to {
                    continue;
                }
                let mut required_counts: HashMap<domain::ItemId, u32> = HashMap::new();
                for id in items {
                    *required_counts.entry(id.clone()).or_insert(0) += 1;
                }
                let can_complete = required_counts
                    .iter()
                    .all(|(id, needed)| player.inventory.count(id) >= *needed);
                if can_complete {
                    for (id, needed) in required_counts {
                        for _ in 0..needed {
                            player.inventory.remove_one(&id);
                        }
                    }
                    quest_msgs.push(complete_legacy_quest(addr, q, player, world, tx));
                    completed_now.push(q.id.clone());
                }
            }
            crate::world::QuestObjective::TalkToNpc { npc: target_npc } => {
                if &npc.id == target_npc {
                    let can_complete = q.id.as_str() != "quest_dragon_eye"
                        || player.inventory.contains(&ItemId::from("dragon_eye"));
                    if can_complete {
                        quest_msgs.push(complete_legacy_quest(addr, q, player, world, tx));
                        completed_now.push(q.id.clone());
                    }
                }
            }
            crate::world::QuestObjective::FetchItem { .. }
            | crate::world::QuestObjective::DefeatNpc { .. } => {
                if let Some(legacy_target) = &q.target_id {
                    if q.giver_id.as_ref() == Some(&npc.id)
                        && player.inventory.contains(legacy_target) {
                        player.inventory.remove_one(legacy_target);
                        quest_msgs.push(complete_legacy_quest(addr, q, player, world, tx));
                        completed_now.push(q.id.clone());
                    }
                }
            }
        }
    }
    Some((quest_msgs, completed_now))
}
