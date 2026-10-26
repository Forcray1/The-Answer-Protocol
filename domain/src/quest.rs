//! Shared types for the multi-stage quest system.
//! Used by the server, console client, and Bevy client.

use serde::{Deserialize, Serialize};
use crate::{ItemId, NpcId, QuestId};

// ── Quest Stage ───────────────────────────────────────────────────────────

/// Quest advancement stage.
///
/// - `NotStarted` : the player hasn't accepted / triggered it yet.
/// - `Stage(n)`   : in progress, step n (1-based).
/// - `Completed`  : reward already given, quest archived.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuestStage {
    NotStarted,
    Stage(u32),
    Completed,
}

impl Default for QuestStage {
    fn default() -> Self {
        QuestStage::NotStarted
    }
}

impl QuestStage {
    /// Advances to the next stage.
    pub fn next(&self) -> QuestStage {
        match self {
            QuestStage::NotStarted => QuestStage::Stage(1),
            QuestStage::Stage(n)   => QuestStage::Stage(n + 1),
            QuestStage::Completed  => QuestStage::Completed,
        }
    }

    /// Deserializes from the wire text format:
    /// `"notstarted"` | `"1"` | `"2"` | … | `"completed"`.
    pub fn from_wire(s: &str) -> Self {
        match s {
            "notstarted" => QuestStage::NotStarted,
            "completed"  => QuestStage::Completed,
            n            => n.parse::<u32>().map(QuestStage::Stage).unwrap_or_default(),
        }
    }

    /// Serializes to the wire text format.
    pub fn to_wire(&self) -> String {
        match self {
            QuestStage::NotStarted => "notstarted".into(),
            QuestStage::Completed  => "completed".into(),
            QuestStage::Stage(n)   => n.to_string(),
        }
    }

    /// Raw stage number (None if NotStarted / Completed).
    pub fn stage_num(&self) -> Option<u32> {
        if let QuestStage::Stage(n) = self { Some(*n) } else { None }
    }
}

// ── Stage Objective ───────────────────────────────────────────────────────

/// Objective attached to a specific quest stage.
///
/// One objective per stage; the server decides when it is fulfilled
/// and advances the stage automatically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StageObjective {
    /// Talk to a specific NPC (triggered on `TALK`).
    TalkToNpc { npc: NpcId },

    /// Have / pick up a specific item in the inventory.
    FetchItem { item: ItemId },

    /// Kill a mob of the given type `count` times.
    KillMob { npc: NpcId, count: u32 },

    /// Give the correct answer in a multiple-choice dialogue.
    /// The client sends `ANSWER <npc_id> <answer>`; the server checks
    /// that `answer == expected_answer` before advancing.
    AnswerDialogue { npc: NpcId, expected_answer: String },
}

// ── Player Progress ───────────────────────────────────────────────────────

/// Tracks a player's progress on a quest.
/// Persisted in DB and sent to the client via `S: EVT QUEST_UPDATE`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestProgress {
    pub quest_id: QuestId,
    /// Current stage.
    pub stage: QuestStage,
    /// Generic counter:
    /// - `KillMob` → number of mobs killed in the current stage.
    /// - other objectives → unused (0).
    #[serde(default)]
    pub counter: u32,
}

impl QuestProgress {
    pub fn new(quest_id: QuestId) -> Self {
        QuestProgress {
            quest_id,
            stage: QuestStage::NotStarted,
            counter: 0,
        }
    }

    pub fn is_completed(&self) -> bool {
        self.stage == QuestStage::Completed
    }
}
