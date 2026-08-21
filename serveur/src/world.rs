// serveur/src/world.rs
//
// Le CATALOGUE statique : world.yaml -> structs Rust. C'est la "définition" des
// choses (ce qu'EST un objet/pnj/lieu), par opposition à leur "placement" runtime
// (serveur/src/state.rs).
//
// Depuis l'adoption du modèle de `domain`, le catalogue n'utilise plus de `String`
// brutes pour les identifiants : il réutilise les newtypes (ItemId, NpcId, RoomId,
// QuestId) et l'énum `Direction`. Les clés du YAML restent des chaînes — les
// newtypes se désérialisent de façon transparente depuis une chaîne, donc
// world.yaml n'a pas besoin de changer.

use serde::Deserialize;
use std::collections::HashMap;

use domain::{Direction, ItemBucket, ItemId, NpcId, QuestId, RoomId};

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
    pub exp_reward: Option<i32>,
    #[serde(default)]
    pub drops: Vec<DropChance>,
    #[serde(default)]
    pub respawn_time: Option<u64>,
    #[serde(default)]
    pub sprite: Option<String>,
    #[serde(default)]
    pub spawn_pos: Option<[f32; 2]>,
    #[serde(default)]
    pub scale: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Stats {
    pub damage: i32,
    pub defense: i32,
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
    pub slot: Option<String>,
    #[serde(default)]
    pub sprite: Option<String>,
}

impl Item {
    /// Dans quel sac d'inventaire ranger cet objet quand on le ramasse.
    /// Les armes (slot "weapon") vont avec les armes ; les objets de quête sans
    /// emplacement sont des objets-clés ; le reste (y compris les armures) va
    /// dans le sac générique.
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

    /// Cet objet est-il une arme équipable ?
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
}

#[derive(Debug, Deserialize, Clone)]
pub struct Quest {
    pub id: QuestId,
    pub name: String,
    pub description: String,
    pub objective: QuestObjective,
    pub target_id: ItemId,
    #[serde(default)]
    pub giver_id: Option<NpcId>,
    pub reward_exp: Option<i32>,
    pub reward_item: Option<ItemId>,
}

impl WorldData {
    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let data: WorldData = serde_yaml::from_str(&content)?;
        Ok(data)
    }
}
