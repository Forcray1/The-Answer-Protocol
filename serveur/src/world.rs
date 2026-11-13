// serveur/src/world.rs
//
// Static CATALOG: world.yaml -> Rust structs. This is the "definition" of
// things (what an item/npc/location IS), as opposed to their runtime "placement"
// (serveur/src/state.rs).
//
// Since adopting the `domain` model, the catalog no longer uses raw `String`
// identifiers: it reuses newtypes (ItemId, NpcId, RoomId,
// QuestId) and the `Direction` enum. YAML keys remain strings —
// newtypes deserialize transparently from a string, so
// world.yaml doesn't need to change.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use domain::{Direction, ItemBucket, ItemId, NpcId, QuestId, RoomId};

// ── PNJ ──────────────────────────────────────────────────────────────────

/// A selectable answer option in a multiple-choice dialogue.
#[derive(Debug, Deserialize, Clone)]
pub struct DialogueChoice {
    /// Text displayed to the player in the UI.
    pub text: String,
    /// Value sent back to the server via `ANSWER <npc_id> <answer>`.
    pub answer: String,
}

/// NPC dialogue block conditioned on a quest + stage.
/// `stage` is 0 for NotStarted (before acceptance),
/// 1..N for `Stage(n)`, and 255 for `Completed`.
#[derive(Debug, Deserialize, Clone)]
pub struct QuestDialogue {
    pub quest_id: QuestId,
    /// 0 = NotStarted, 1+ = Stage(n), 255 = Completed.
    pub stage: u32,
    pub lines: Vec<String>,
    #[serde(default)]
    pub choices: Vec<DialogueChoice>,
}

// ── QUÊTE ─────────────────────────────────────────────────────────────────

/// Objective associated with a specific quest stage (world.yaml).
/// Re-exports the domain type so world.rs can use it directly.
pub use domain::StageObjective as StagedObjective;

/// Behaviour when the player gives a wrong answer to an `AnswerDialogue`.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct OnWrongAnswer {
    /// Dialogue lines to send to the player after a rejection.
    #[serde(default)]
    pub dialogue: Vec<String>,
    /// If `true`, quest progress is removed (reset to NotStarted).
    /// The player can restart the conversation from scratch.
    #[serde(default)]
    pub reset: bool,
}

/// Definition of a quest stage in the YAML.
#[derive(Debug, Deserialize, Clone)]
pub struct QuestStageDef {
    /// Stage number (1-based).
    pub stage: u32,
    /// Objective to fulfill to advance to the next stage (or `null`).
    #[serde(default)]
    pub objective: Option<StagedObjective>,
    /// Behaviour on wrong answer (only for `AnswerDialogue`).
    #[serde(default)]
    pub on_wrong_answer: Option<OnWrongAnswer>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WorldData {
    pub world: World,
}

#[derive(Debug, Deserialize, Clone)]
pub struct World {
    pub name: String,
    pub npcs: Vec<Npc>,
    pub items: Vec<Item>,
    pub locations: HashMap<RoomId, Location>,
    pub quests: Vec<Quest>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DropChance {
    pub item_id: ItemId,
    pub chance: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Npc {
    pub id: NpcId,
    pub name: String,
    pub role: String,
    pub hp: i32,
    pub dialogue: Vec<String>,
    #[serde(default)]
    pub damage: Option<i32>,
    #[serde(default)]
    pub defense: Option<i32>,
    #[serde(default)]
    pub special_damage: Option<i32>,
    #[serde(default)]
    pub special_defense: Option<i32>,
    #[serde(default)]
    pub exp_reward: Option<i32>,
    #[serde(default)]
    pub drops: Vec<DropChance>,
    #[serde(default)]
    pub respawn_time: Option<u64>,
    #[serde(default, alias = "map_sprite")]
    pub sprite: Option<String>,
    #[serde(default, alias = "battle_sprite")]
    pub combat_sprite: Option<String>,
    #[serde(default)]
    pub spawn_pos: Option<[f32; 2]>,
    #[serde(default)]
    pub scale: Option<f32>,
    /// Dialogue blocks conditional on quest state.
    #[serde(default)]
    pub quest_dialogues: Vec<QuestDialogue>,
    /// Items sold by this NPC (empty = not a merchant).
    #[serde(default)]
    pub shop: Vec<ShopItem>,
}

/// One entry in an NPC's shop list (parsed from world.yaml).
#[derive(Debug, Deserialize, Clone)]
pub struct ShopItem {
    pub item_id: ItemId,
    pub price: u32,
}

impl Npc {
    pub fn map_sprite(&self) -> &str {
        self.sprite.as_deref().unwrap_or("default")
    }

    pub fn combat_sprite(&self) -> &str {
        self.combat_sprite
            .as_deref()
            .or(self.sprite.as_deref())
            .unwrap_or("default")
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ItemType {
    Standard,
    Quest,
}

fn default_item_type() -> ItemType {
    ItemType::Standard
}

#[derive(Debug, Deserialize, Clone)]
pub struct Item {
    pub id: ItemId,
    pub name: String,
    pub description: String,
    pub obtainable: bool,
    #[serde(default = "default_item_type")]
    pub r#type: ItemType,
    #[serde(default)]
    pub damage: Option<i32>,
    #[serde(default)]
    pub defense: Option<i32>,
    #[serde(default)]
    pub special_damage: Option<i32>,
    #[serde(default)]
    pub special_defense: Option<i32>,
    #[serde(default)]
    pub slot: Option<String>,
    #[serde(default)]
    pub sprite: Option<String>,
    /// If true, this item can be bought multiple times (e.g. potions).
    #[serde(default)]
    pub consumable: bool,
    /// Heal amount (as % of max HP, e.g. 50 = heal 50% max HP). Only used if consumable = true.
    #[serde(default)]
    pub heal_percent: Option<u32>,
}

impl Item {
    /// Which inventory bag to put this item in when picked up.
    /// Weapons (slot "weapon") go in the weapon bag; quest items without
    /// a slot are key items; the rest (including armor) go
    /// in the generic bag.
    pub fn bucket(&self) -> ItemBucket {
        if self.slot.as_deref() == Some("weapon") {
            ItemBucket::Weapon
        } else if self.slot.is_some() {
            ItemBucket::Item
        } else if self.r#type == ItemType::Quest {
            ItemBucket::KeyItem
        } else {
            ItemBucket::Item
        }
    }

    /// Is this item an equippable weapon?
    pub fn is_weapon(&self) -> bool {
        self.slot.as_deref() == Some("weapon")
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Location {
    pub name: String,
    pub description: String,
    pub exits: HashMap<Direction, RoomId>,
    #[serde(default)]
    pub npcs: Vec<NpcId>,
    #[serde(default)]
    pub items: Vec<ItemId>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QuestObjective {
    FetchItem { item: ItemId },
    DefeatNpc { npc: NpcId },
    DeliverItem { item: ItemId, to: NpcId },
    DeliverItems { items: Vec<ItemId>, to: NpcId },
    TalkToNpc { npc: NpcId },
}

#[derive(Debug, Deserialize, Clone)]
pub struct Quest {
    pub id: QuestId,
    pub name: String,
    pub description: String,
    /// Single objective (legacy format, compatible with existing YAMLs).
    pub objective: QuestObjective,
    #[serde(default)]
    pub target_id: Option<ItemId>,
    #[serde(default)]
    pub giver_id: Option<NpcId>,
    pub reward_exp: Option<i32>,
    pub reward_item: Option<ItemId>,
    /// Gold given on completion.
    #[serde(default)]
    pub reward_gold: Option<i32>,
    /// Extra reward items; list an id N times to give N units.
    #[serde(default)]
    pub reward_items: Vec<ItemId>,
    /// Stage definitions (new multi-step format).
    /// If empty, the quest has a single implicit stage (Stage 1).
    #[serde(default)]
    pub stages: Vec<QuestStageDef>,
}

impl WorldData {
    fn validate_quest_objective(
        objective: &QuestObjective,
        item_ids: &HashSet<ItemId>,
        npc_ids: &HashSet<NpcId>,
    ) -> Result<(), String> {
        match objective {
            QuestObjective::FetchItem { item } => {
                if !item_ids.contains(item) {
                    return Err(format!("Quest references unknown item '{item}'"));
                }
            }
            QuestObjective::DefeatNpc { npc } => {
                if !npc_ids.contains(npc) {
                    return Err(format!("Quest references unknown NPC '{npc}'"));
                }
            }
            QuestObjective::DeliverItem { item, to } => {
                if !item_ids.contains(item) {
                    return Err(format!("Quest references unknown item '{item}'"));
                }
                if !npc_ids.contains(to) {
                    return Err(format!("Quest references unknown recipient NPC '{to}'"));
                }
            }
            QuestObjective::DeliverItems { items, to } => {
                for item in items {
                    if !item_ids.contains(item) {
                        return Err(format!("Quest references unknown item '{item}'"));
                    }
                }
                if !npc_ids.contains(to) {
                    return Err(format!("Quest references unknown recipient NPC '{to}'"));
                }
            }
            QuestObjective::TalkToNpc { npc } => {
                if !npc_ids.contains(npc) {
                    return Err(format!("Quest references unknown NPC '{npc}'"));
                }
            }
        }
        Ok(())
    }

    fn validate_stage_objective(
        objective: &domain::StageObjective,
        item_ids: &HashSet<ItemId>,
        npc_ids: &HashSet<NpcId>,
    ) -> Result<(), String> {
        match objective {
            domain::StageObjective::FetchItem { item } => {
                if !item_ids.contains(item) {
                    return Err(format!("Quest stage references unknown item '{item}'"));
                }
            }
            domain::StageObjective::TalkToNpc { npc } => {
                if !npc_ids.contains(npc) {
                    return Err(format!("Quest stage references unknown NPC '{npc}'"));
                }
            }
            domain::StageObjective::KillMob { npc, .. } => {
                if !npc_ids.contains(npc) {
                    return Err(format!("Quest stage references unknown NPC '{npc}'"));
                }
            }
            domain::StageObjective::AnswerDialogue { npc, .. } => {
                if !npc_ids.contains(npc) {
                    return Err(format!("Quest stage references unknown NPC '{npc}'"));
                }
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        let item_ids: HashSet<_> = self.world.items.iter().map(|item| item.id.clone()).collect();
        let npc_ids: HashSet<_> = self.world.npcs.iter().map(|npc| npc.id.clone()).collect();
        let room_ids: HashSet<_> = self.world.locations.keys().cloned().collect();

        if room_ids.is_empty() {
            return Err("World contains no rooms.".to_string());
        }

        let start_room = RoomId::from("Start_oasis");
        if !room_ids.contains(&start_room) {
            return Err("World must define a room named 'Start_oasis'.".to_string());
        }

        for (room_id, location) in &self.world.locations {
            for (direction, exit_room) in &location.exits {
                if !room_ids.contains(exit_room) {
                    return Err(format!(
                        "Room '{room_id}' exits to unknown room '{exit_room}' via {direction:?}."
                    ));
                }
            }

            for npc_id in &location.npcs {
                if !npc_ids.contains(npc_id) {
                    return Err(format!(
                        "Room '{room_id}' references unknown NPC '{npc_id}'."
                    ));
                }
            }

            for item_id in &location.items {
                if !item_ids.contains(item_id) {
                    return Err(format!(
                        "Room '{room_id}' references unknown item '{item_id}'."
                    ));
                }
            }
        }

        for npc in &self.world.npcs {
            for drop in &npc.drops {
                if !item_ids.contains(&drop.item_id) {
                    return Err(format!(
                        "NPC '{}' drops unknown item '{}'.",
                        npc.id, drop.item_id
                    ));
                }
            }
        }

        for quest in &self.world.quests {
            if let Some(giver_id) = &quest.giver_id {
                if !npc_ids.contains(giver_id) {
                    return Err(format!(
                        "Quest '{}' references unknown giver NPC '{}'.",
                        quest.id, giver_id
                    ));
                }
            }
            if let Some(target_id) = &quest.target_id {
                if !item_ids.contains(target_id) {
                    return Err(format!(
                        "Quest '{}' references unknown target item '{}'.",
                        quest.id, target_id
                    ));
                }
            }

            Self::validate_quest_objective(&quest.objective, &item_ids, &npc_ids)?;

            for item in &quest.reward_items {
                if !item_ids.contains(item) {
                    return Err(format!(
                        "Quest '{}' rewards unknown item '{}'.",
                        quest.id, item
                    ));
                }
            }

            if let Some(item) = &quest.reward_item {
                if !item_ids.contains(item) {
                    return Err(format!(
                        "Quest '{}' rewards unknown item '{}'.",
                        quest.id, item
                    ));
                }
            }

            for stage in &quest.stages {
                if let Some(objective) = &stage.objective {
                    Self::validate_stage_objective(objective, &item_ids, &npc_ids)?;
                }
            }
        }

        Ok(())
    }

    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let data: WorldData = serde_yaml::from_str(&content)?;
        data.validate().map_err(|err| -> Box<dyn std::error::Error> { err.into() })?;
        Ok(data)
    }
}
