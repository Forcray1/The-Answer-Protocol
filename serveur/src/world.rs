// serveur/src/world.rs

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct WorldData {
    pub world: World,
}

#[derive(Debug, Deserialize, Clone)]
pub struct World {
    pub name: String,
    pub npcs: Vec<Npc>,
    pub items: Vec<Item>,
    pub locations: HashMap<String, Location>,
    pub quests: Vec<Quest>,
}


#[derive(Debug, Deserialize, Clone)]
pub struct DropChance {
    pub item_id: String,
    pub chance: u32,
}
#[derive(Debug, Deserialize, Clone)]
pub struct Npc {
    pub id: String,
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
    pub id: String,
    pub name: String,
    pub description: String,
    pub obtainable: bool,
    #[serde(default = "default_item_type")]
    pub r#type: ItemType,
    #[serde(default)]
    pub damage: Option<i32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Location {
    pub name: String,
    pub description: String,
    pub exits: HashMap<String, String>,
    #[serde(default)]
    pub npcs: Vec<String>,
    #[serde(default)]
    pub items: Vec<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QuestObjective {
	FetchItem { item: String },
	DefeatNpc { npc: String },
	DeliverItem { item: String, to: String },
}

#[derive(Debug, Deserialize, Clone)]
pub struct Quest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub objective: QuestObjective,
    pub target_id: String,
    #[serde(default)]
    pub giver_id: Option<String>,
    pub reward_exp: Option<i32>,
    pub reward_item: Option<String>,
}

impl WorldData {
    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let data: WorldData = serde_yaml::from_str(&content)?;
        Ok(data)
    }
}