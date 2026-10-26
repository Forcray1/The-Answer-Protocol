//! Quest system shared types.
//!
//! This module provides:
//! - QuestStage, StageObjective, QuestProgress types used by server and clients.
//! - Listens to `S: EVT QUEST_UPDATE` server events to
//!   update the tracker.
//! - Syncs the UI (QuestPlugin in ui.rs) by sending
//!   the `QUESTS` command after each update.

use bevy::prelude::*;
use domain::quest::{QuestProgress, QuestStage};

use crate::net::{NetworkSender, ServerMessageEvent};
use crate::AppState;

pub struct QuestTrackerPlugin;

impl Plugin for QuestTrackerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<QuestTracker>()
            .add_event::<QuestUpdatedEvent>()
            .add_systems(
                Update,
                (
                    handle_quest_update_events,
                    sync_quest_ui_on_update,
                )
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

// ── Resource ──────────────────────────────────────────────────────────────

/// Holds the list of active quests for the local player.
#[derive(Resource, Default)]
pub struct QuestTracker {
    pub quests: Vec<QuestProgress>,
}

impl QuestTracker {
    /// Updates or inserts quest progress.
    pub fn upsert(&mut self, quest_id: &str, stage: QuestStage, counter: u32) {
        let id = domain::QuestId::from(quest_id);
        if let Some(prog) = self.quests.iter_mut().find(|p| p.quest_id == id) {
            prog.stage = stage.clone();
            prog.counter = counter;
        } else {
            self.quests.push(QuestProgress {
                quest_id: id,
                stage,
                counter,
            });
        }
        // Remove completed quests from the active tracker.
        self.quests.retain(|p| p.stage != QuestStage::Completed);
    }

    /// Returns the current stage of a quest (None if unknown).
    pub fn stage_of(&self, quest_id: &str) -> Option<&QuestStage> {
        let id = domain::QuestId::from(quest_id);
        self.quests.iter().find(|p| p.quest_id == id).map(|p| &p.stage)
    }
}

// ── Internal Event ────────────────────────────────────────────────────────

/// Fired every time a quest changes stage.
#[derive(Event)]
pub struct QuestUpdatedEvent {
    pub quest_id: String,
    pub stage: QuestStage,
    pub counter: u32,
}

// ── Systems ───────────────────────────────────────────────────────────────

/// Parses `S: EVT QUEST_UPDATE <quest_id> <stage> <counter>`
/// and `S: EVT QUEST_RESET <quest_id>` messages, updating the `QuestTracker`.
fn handle_quest_update_events(
    mut server_events: EventReader<ServerMessageEvent>,
    mut tracker: ResMut<QuestTracker>,
    mut quest_updated: EventWriter<QuestUpdatedEvent>,
) {
    for ev in server_events.read() {
        // Quest reset (wrong answer with reset flag)
        if let Some(quest_id) = ev.0.strip_prefix("S: EVT QUEST_RESET ") {
            let id = domain::QuestId::from(quest_id.trim());
            tracker.quests.retain(|p| p.quest_id != id);
            continue;
        }

        let Some(rest) = ev.0.strip_prefix("S: EVT QUEST_UPDATE ") else {
            continue;
        };
        // Format : `<quest_id> <stage_wire> <counter>`
        let mut parts = rest.splitn(3, ' ');
        let Some(quest_id) = parts.next() else { continue };
        let Some(stage_str) = parts.next() else { continue };
        let counter: u32 = parts.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);

        let stage = QuestStage::from_wire(stage_str);
        tracker.upsert(quest_id, stage.clone(), counter);
        quest_updated.send(QuestUpdatedEvent {
            quest_id: quest_id.to_string(),
            stage,
            counter,
        });
    }
}

/// After a quest update, re-requests the full quest list
/// from the server to refresh the UI (QuestPlugin in ui.rs).
fn sync_quest_ui_on_update(
    mut quest_updated: EventReader<QuestUpdatedEvent>,
    sender: Option<Res<NetworkSender>>,
) {
    let Some(sender) = sender else { return };
    for _ in quest_updated.read() {
        let _ = sender.0.try_send("QUESTS\n".to_string());
    }
}
