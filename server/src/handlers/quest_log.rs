//! `QUEST` / `QUESTS`: the quest log texts.

use std::net::SocketAddr;
use serde_json::json;

use domain::quest::QuestProgress;

use crate::state::ServerState;
use crate::world::WorldData;
use crate::handlers::quest_core::{group_item_ids, quest_progress_for};
use crate::handlers::util::ci_eq;

/// Human-readable objective of a quest (English). `prog` is the player's progress,
/// or `None` to describe the first stage of a quest the player has not started.
/// The text must not contain ':' or '|' (field separators of EVT QUEST_DATA).
pub(crate) fn quest_objective_text(
    q: &crate::world::Quest,
    prog: Option<&QuestProgress>,
    world: &WorldData,
) -> String {
    use crate::world::QuestObjective as Legacy;
    use domain::quest::StageObjective as Obj;

    let label = |id: &domain::ItemId| -> String {
        world.item_by_id(id)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| id.to_string())
    };

    if q.stages.is_empty() {
        // Legacy single-objective quest.
        return match &q.objective {
            Legacy::FetchItem { item } => format!("Fetch {}", label(item)),
            Legacy::DefeatNpc { npc } => format!("Defeat {}", npc),
            Legacy::DeliverItem { item, to } => format!("Bring {} to {}", label(item), to),
            Legacy::DeliverItems { items, to } => {
                // Group repeated ids: [bone, bone, diamond] -> "2x Bone, 1x Diamond"
                let list = group_item_ids(items)
                    .iter()
                    .map(|(id, n)| format!("{}x {}", n, label(id)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("Bring {} to {}", list, to)
            }
            Legacy::TalkToNpc { npc } => format!("Talk to {}", npc),
        };
    }

    // Multi-stage quest.
    let (stage_num, counter) = match prog {
        Some(p) => match p.stage.stage_num() {
            Some(n) => (n, p.counter),
            None => return p.stage.to_wire(),
        },
        None => (1, 0),
    };
    match q
        .stages
        .iter()
        .find(|s| s.stage == stage_num)
        .and_then(|s| s.objective.as_ref()) {
        Some(Obj::TalkToNpc { npc }) => format!("Talk to {} (stage {})", npc, stage_num),
        Some(Obj::FetchItem { item }) => format!(
            "Bring {} to the quest giver (stage {})",
            label(item),
            stage_num
        ),
        Some(Obj::KillMob { npc, count }) => format!(
            "Defeat {} x{} ({}/{}) (stage {})",
            npc, count, counter, count, stage_num
        ),
        Some(Obj::AnswerDialogue { npc, .. }) => format!("Answer {} (stage {})", npc, stage_num),
        None => format!("Stage {}", stage_num),
    }
}

/// For delivery quests: what the player holds vs what is needed, e.g. "Bone 4/10".
/// Empty for every other kind of quest. Must not contain ':' or '|'.
pub(crate) fn delivery_progress(
    q: &crate::world::Quest,
    player: &crate::state::Player,
    world: &WorldData,
) -> String {
    use crate::world::QuestObjective as Legacy;
    if !q.stages.is_empty() {
        return String::new();
    }
    let mut needed: Vec<(domain::ItemId, u32)> = Vec::new();
    match &q.objective {
        Legacy::DeliverItem { item, .. } => needed.push((item.clone(), 1)),
        Legacy::DeliverItems { items, .. } => {
            for i in items {
                let mut found = false;
                for entry in needed.iter_mut() {
                    if &entry.0 == i {
                        entry.1 += 1;
                        found = true;
                        break;
                    }
                }
                if !found {
                    needed.push((i.clone(), 1));
                }
            }
        }
        _ => return String::new(),
    }
    needed
        .iter()
        .map(|(id, need)| {
            let name = world.item_by_id(id)
                .map(|i| i.name.clone())
                .unwrap_or_else(|| id.to_string());
            let have = player.inventory.count(id).min(*need);
            format!("{} {}/{}", name, have, need)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Short counter shown in the journal: kills (`7/9`) or items to bring (`4/10`).
/// Empty for quests without anything to count.
fn quest_progress_label(
    q: &crate::world::Quest,
    prog: &QuestProgress,
    player: &crate::state::Player,
) -> String {
    use crate::world::QuestObjective as Legacy;
    use domain::quest::StageObjective as Obj;

    if q.stages.is_empty() {
        let needed = match &q.objective {
            Legacy::DeliverItem { item, .. } => vec![(item.clone(), 1)],
            Legacy::DeliverItems { items, .. } => group_item_ids(items),
            _ => return String::new(),
        };
        let total: u32 = needed.iter().map(|(_, need)| *need).sum();
        let have: u32 = needed
            .iter()
            .map(|(id, need)| player.inventory.count(id).min(*need))
            .sum();
        return format!("{}/{}", have, total);
    }
    let Some(stage_num) = prog.stage.stage_num() else {
        return String::new();
    };
    match q
        .stages
        .iter()
        .find(|s| s.stage == stage_num)
        .and_then(|s| s.objective.as_ref()) {
        Some(Obj::KillMob { count, .. }) => format!("{}/{}", prog.counter, count),
        _ => String::new(),
    }
}

/// One `EVT QUEST_DATA` entry per active quest, board quest included:
/// `<id>:<name>:<description>:<objective>:<stage>:<counter>`.
fn quest_log_entries(
    player: &crate::state::Player,
    state: &ServerState,
    world: &WorldData,
) -> Vec<String> {
    let mut parts = Vec::new();
    for prog in &player.quest_progress {
        if let Some(q) = world.world.quests.iter().find(|q| q.id == prog.quest_id) {
            let mut objective_str = quest_objective_text(q, Some(prog), world);
            let progress = delivery_progress(q, player, world);
            if !progress.is_empty() {
                objective_str.push_str(&format!(" - you have {}", progress));
            }
            parts.push(format!(
                "{}:{}:{}:{}:{}:{}",
                q.id,
                q.name,
                q.description,
                objective_str,
                prog.stage.to_wire(),
                quest_progress_label(q, prog, player)
            ));
        }
    }

    // Append active board quest if the player holds one.
    if let Some(slot) = state.board_quests.player_slot(&player.username) {
        let desc = format!(
            "Kill {} {} {}. Reward: {} gold.",
            slot.kill_count, slot.mob_name, slot.location_hint, slot.gold_reward
        );
        let objective = format!(
            "Kill {} {} ({}/{})",
            slot.kill_count, slot.mob_name, slot.kills_done, slot.kill_count
        );
        parts.push(format!(
            "board_quest_{}:Board Quest:{}:{}:1:{}/{}",
            slot.slot_id + 1,
            desc,
            objective,
            slot.kills_done,
            slot.kill_count
        ));
    }
    parts
}

fn format_quest_data(entries: &[String]) -> String {
    if entries.is_empty() {
        "S: EVT QUEST_DATA empty\n".to_string()
    } else {
        format!("S: EVT QUEST_DATA {}\n", entries.join("|"))
    }
}

/// The player's full quest journal, ready to send.
pub(crate) fn quest_data_event(
    player: &crate::state::Player,
    state: &ServerState,
    world: &WorldData,
) -> String {
    format_quest_data(&quest_log_entries(player, state, world))
}

/// `QUEST <id or name>`: details and status of one quest.
/// RFC 42TAP: OK { "id": ..., "name": ..., "status": ..., ... } or ERR unknown_quest.
pub(crate) fn handle_quest(addr: SocketAddr, query: String, state: &ServerState, world: &WorldData) -> String {
    let Some(player) = state.players.get(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    let normalized_query = query.trim();
    let quest_opt = if normalized_query.is_empty() {
        player
            .quest_progress
            .first()
            .and_then(|progress| world.world.quests.iter().find(|q| q.id == progress.quest_id))
    } else {
        world
            .world
            .quests
            .iter()
            .find(|q| ci_eq(q.id.as_str(), normalized_query) || ci_eq(&q.name, normalized_query))
    };
    let Some(q) = quest_opt else {
        return if normalized_query.is_empty() {
            "S: ERR no_active_quest\n".to_string()
        } else {
            "S: ERR unknown_quest\n".to_string()
        };
    };

    let prog = quest_progress_for(player, &q.id);
    let (status, stage) = if player.completed_quests.contains(&q.id) {
        ("completed", "completed".to_string())
    } else if let Some(p) = prog {
        ("active", p.stage.to_wire())
    } else {
        ("not_started", "notstarted".to_string())
    };
    let objective = if status == "completed" {
        "Completed".to_string()
    } else {
        quest_objective_text(q, prog, world)
    };
    let progress = if status == "completed" {
        String::new()
    } else {
        delivery_progress(q, player, world)
    };
    let reward_items: Vec<String> = q.reward_items.iter().map(|i| i.to_string()).collect();

    let reply = json!({
        "id": q.id.as_str(),
        "name": q.name,
        "description": q.description,
        "status": status,
        "stage": stage,
        "objective": objective,
        "progress": progress,
        "giver": q.giver_id.as_ref().map(|g| g.to_string()),
        "reward_exp": q.reward_exp,
        "reward_gold": q.reward_gold,
        "reward_items": reward_items,
        "reward_item": q.reward_item.as_ref().map(|i| i.to_string()),
    });
    format!("S: OK {}\n", reply)
}

pub(crate) fn handle_quests(addr: SocketAddr, state: &ServerState, world: &WorldData) -> String {
    let Some(player) = state.players.get(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    let entries = quest_log_entries(player, state, world);
    format!("S: OK quests={}\n{}", entries.len(), format_quest_data(&entries))
}
