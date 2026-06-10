// serveur/src/world.rs

use serde::Deserialize;
use std::collections::HashMap;

// Représente l'intégralité du fichier YAML
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
pub struct Npc {
    pub id: String,
    pub name: String,
    pub role: String,
    pub hp: i32,
    pub dialogue: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub description: String,
    pub obtainable: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Location {
    pub name: String,
    pub description: String,
    pub exits: HashMap<String, String>, // Exemple : {"north": "foret_sombre"}
    #[serde(default)]
    pub npcs: Vec<String>,              // Liste des IDs des NPCs présents
    #[serde(default)]
    pub items: Vec<String>,             // Liste des IDs des objets au sol
}

#[derive(Debug, Deserialize, Clone)]
pub struct Quest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub r#type: String, // "type" est un mot-clé réservé en Rust, on utilise r#type
    pub target_id: String,
    #[serde(default)]
    pub giver_id: Option<String>,
    pub reward_exp: Option<i32>,
    pub reward_item: Option<String>,
}

impl WorldData {
    /// Charge et parse le fichier YAML passé en paramètre
    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let data: WorldData = serde_yaml::from_str(&content)?;
        Ok(data)
    }
}