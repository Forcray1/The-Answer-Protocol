//! Quest bookkeeping shared by dialogue and combat: progress, stages, rewards.

use std::net::SocketAddr;
use tokio::sync::broadcast::Sender;

use domain::quest::{QuestProgress, QuestStage};
use domain::ItemBucket;

use crate::state::{Player, RuntimeItem};
use crate::world::{ItemType, WorldData};
use crate::events::{to_player, GlobalEvent};
use crate::handlers::player_events::player_stats_event;
use crate::handlers::util::ci_eq;

/// Tells the client a quest ended and refreshes the gold, XP and HP its rewards changed.
fn quest_completed_event(q: &crate::world::Quest, player: &Player) -> String {
    format!(
        "S: EVT QUEST_UPDATE {} completed 0\nS: EVT GOLD {}\n{}",
        q.id,
        player.money,
        player_stats_event(player)
    )
}

/// True when this quest item on the ground is already done for `player`: taken, held,
/// worn or handed in. Survives a server restart, unlike `collected_by` alone.
pub(crate) fn quest_item_collected(
    player: &Player,
    ground_item: &RuntimeItem,
    world: &WorldData,
) -> bool {
    let id = &ground_item.item_id;
    if world.item_by_id(id).is_none_or(|item| item.r#type != ItemType::Quest) {
        return false;
    }
    ground_item.collected_by.contains(&player.username)
        || player.inventory.contains(id)
        || player.equipped_medaillon_id.as_ref() == Some(id)
        || player.equipement.equipped().iter().any(|(_, worn, _)| worn == id)
        || world.world.quests.iter().any(|q| {
            player.completed_quests.contains(&q.id) && quest_requires_item(q, id)
        })
}

fn quest_requires_item(q: &crate::world::Quest, id: &domain::ItemId) -> bool {
    use crate::world::QuestObjective as Legacy;
    use domain::quest::StageObjective;
    let in_objective = match &q.objective {
        Legacy::FetchItem { item } | Legacy::DeliverItem { item, .. } => item == id,
        Legacy::DeliverItems { items, .. } => items.contains(id),
        _ => false,
    };
    in_objective
        || q.stages.iter().any(|stage| {
            matches!(&stage.objective, Some(StageObjective::FetchItem { item }) if item == id)
        })
}

/// Counts a kill for the player's active "kill" stages, advancing or completing them.
/// Returns the lines to send back (`S: OK [QUEST] <name> 7/9`, reward messages).
pub(crate) fn record_kill_for_quests(
    addr: SocketAddr,
    mob_id: &domain::NpcId,
    player: &mut Player,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    let mut out = String::new();
    let mut to_advance: Vec<domain::QuestId> = Vec::new();
    let mut to_complete: Vec<domain::QuestId> = Vec::new();
    for prog in &mut player.quest_progress {
        let QuestStage::Stage(stage_num) = prog.stage else {
            continue;
        };
        let Some(q) = world.world.quests.iter().find(|q| q.id == prog.quest_id) else {
            continue;
        };
        let objective = q
            .stages
            .iter()
            .find(|s| s.stage == stage_num)
            .and_then(|s| s.objective.as_ref());
        let Some(domain::quest::StageObjective::KillMob { npc, count }) = objective else {
            continue;
        };
        if !ci_eq(npc.as_str(), mob_id.as_str()) {
            continue;
        }
        prog.counter += 1;
        out.push_str(&format!("S: OK [QUEST] {} {}/{}\n", q.name, prog.counter, count));
        if prog.counter >= *count {
            if q.stages.iter().all(|s| s.stage <= stage_num) {
                to_complete.push(prog.quest_id.clone());
            } else {
                to_advance.push(prog.quest_id.clone());
            }
        }
    }
    for qid in to_advance {
        advance_quest_stage(addr, &qid, player, tx);
    }
    for qid in to_complete {
        if let Some(q) = world.world.quests.iter().find(|q| q.id == qid) {
            let msg = complete_staged_quest(addr, q, player, world, tx);
            out.push_str(&format!("S: OK {}\n", msg));
        }
    }
    out
}

pub(crate) fn reward_quest(player: &mut Player, q: &crate::world::Quest, world: &WorldData) -> String {
    if !player.completed_quests.contains(&q.id) {
        player.completed_quests.push(q.id.clone());
    }

    let item_label = |id: &domain::ItemId| -> String {
        world.item_by_id(id)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| id.to_string())
    };
    let bucket_of = |id: &domain::ItemId| -> ItemBucket {
        world.item_by_id(id)
            .map(|i| i.bucket())
            .unwrap_or(ItemBucket::Item)
    };

    let mut msg = format!("[QUEST COMPLETE] {}!", q.name);
    if let Some(ref reward_id) = q.reward_item {
        player
            .inventory
            .add(reward_id.clone(), bucket_of(reward_id));
        msg.push_str(&format!(" You receive: {}.", item_label(reward_id)));
    }

    // Extra reward items: an id listed N times gives N units.
    for id in &q.reward_items {
        player.inventory.add(id.clone(), bucket_of(id));
    }
    for (id, n) in &group_item_ids(&q.reward_items) {
        msg.push_str(&format!(" You receive: {}x {}.", n, item_label(id)));
    }

    if let Some(gold) = q.reward_gold {
        player.add_money(gold);
        msg.push_str(&format!(" (+{} gold)", gold));
    }
    if let Some(xp) = q.reward_exp {
        player.add_xp_with_world(xp, world);
        msg.push_str(&format!(" (+{} EXP)", xp));
    }

    msg
}

/// Completes a single-objective quest: archives its progress, gives the reward
/// and tells the client (`EVT QUEST_UPDATE <id> completed 0`).
pub(crate) fn complete_legacy_quest(
    addr: SocketAddr,
    q: &crate::world::Quest,
    player: &mut Player,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    player.quest_progress.retain(|p| p.quest_id != q.id);
    let msg = reward_quest(player, q, world);
    to_player(
        tx,
        quest_completed_event(q, player),
        addr,
    );
    msg
}

/// Finds a player's progress for a given quest.
#[allow(dead_code)]
pub(crate) fn quest_progress_for<'a>(
    player: &'a crate::state::Player,
    quest_id: &domain::QuestId,
) -> Option<&'a QuestProgress> {
    player
        .quest_progress
        .iter()
        .find(|p| &p.quest_id == quest_id)
}

pub(crate) fn quest_progress_for_mut<'a>(
    player: &'a mut crate::state::Player,
    quest_id: &domain::QuestId,
) -> Option<&'a mut QuestProgress> {
    player
        .quest_progress
        .iter_mut()
        .find(|p| &p.quest_id == quest_id)
}

/// Advances a quest stage and sends EVT QUEST_UPDATE to the player.
pub(crate) fn advance_quest_stage(
    addr: SocketAddr,
    quest_id: &domain::QuestId,
    player: &mut crate::state::Player,
    tx: &Sender<GlobalEvent>,
) {
    if let Some(prog) = quest_progress_for_mut(player, quest_id) {
        prog.stage = prog.stage.next();
        prog.counter = 0;
        let stage_wire = prog.stage.to_wire();
        to_player(
            tx,
            format!("S: EVT QUEST_UPDATE {} {} 0\n", quest_id, stage_wire),
            addr,
        );
    }
}

/// Starts a quest (stage NotStarted → Stage 1) if not already tracked.
pub(crate) fn start_quest(
    addr: SocketAddr,
    quest: &crate::world::Quest,
    player: &mut crate::state::Player,
    tx: &Sender<GlobalEvent>,
) {
    if player.completed_quests.contains(&quest.id) {
        return;
    }
    if player.quest_progress.iter().any(|p| p.quest_id == quest.id) {
        return;
    }
    let mut prog = QuestProgress::new(quest.id.clone());
    prog.stage = QuestStage::Stage(1);
    player.quest_progress.push(prog);
    to_player(
        tx,
        format!("S: EVT QUEST_UPDATE {} 1 0\n", quest.id),
        addr,
    );
}

/// Groups repeated ids, keeping first-appearance order: `[bone, bone, gem]` -> `[(bone, 2), (gem, 1)]`.
pub(crate) fn group_item_ids(ids: &[domain::ItemId]) -> Vec<(domain::ItemId, u32)> {
    let mut grouped: Vec<(domain::ItemId, u32)> = Vec::new();
    for id in ids {
        if let Some(pos) = grouped.iter().position(|(known, _)| known == id) {
            grouped[pos].1 += 1;
        } else {
            grouped.push((id.clone(), 1));
        }
    }
    grouped
}

/// Completes a multi-stage quest: archives its progress, gives the reward and tells the
/// client (`EVT QUEST_UPDATE <id> completed 0`). Returns the reward message.
pub(crate) fn complete_staged_quest(
    addr: SocketAddr,
    q: &crate::world::Quest,
    player: &mut Player,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> String {
    if let Some(prog) = quest_progress_for_mut(player, &q.id) {
        prog.stage = QuestStage::Completed;
    }
    player.completed_quests.push(q.id.clone());
    player.quest_progress.retain(|p| p.quest_id != q.id);
    let msg = reward_quest(player, q, world);
    to_player(
        tx,
        quest_completed_event(q, player),
        addr,
    );
    msg
}
