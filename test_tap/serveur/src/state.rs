// serveur/src/state.rs

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;
use crate::world::{WorldData, ItemType};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub enum ItemSource {
    World, 
    MobDrop, 
    PlayerDrop, 
}

#[derive(Debug, Clone)]
pub struct RuntimeItem {
    pub item_id: String,
    pub source: ItemSource,
    pub collected_by: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct Player {
    pub username: String,
    pub hp: i32,
    pub exp: i32,
    pub current_room: String,
    pub inventory: Vec<String>,
    pub completed_quests: Vec<String>,
    pub last_move: Option<Instant>,   
    pub last_attack: Option<Instant>, 
    pub last_chat: Option<Instant>,
}

pub struct ServerState {
    pub players: HashMap<SocketAddr, Player>,
    pub room_items: HashMap<String, Vec<RuntimeItem>>,
    pub room_npcs: HashMap<String, Vec<String>>,
    pub npc_hps: HashMap<String, i32>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            room_items: HashMap::new(),
            room_npcs: HashMap::new(),
            npc_hps: HashMap::new(),
        }
    }

    pub fn initialize_from_world(&mut self, world_data: &WorldData) {
        // 1. On peuple les pièces avec leurs objets et PNJ initiaux
        for (room_id, location) in &world_data.world.locations {
            
            // On convertit les simples chaînes d'ID du YAML en RuntimeItems complexes
            let runtime_items: Vec<RuntimeItem> = location.items.iter().map(|id| RuntimeItem {
                item_id: id.clone(),
                source: ItemSource::World,
                collected_by: HashSet::new(),
            }).collect();

            self.room_items.insert(room_id.clone(), runtime_items);
            self.room_npcs.insert(room_id.clone(), location.npcs.clone());
        }

        // 2. On instancie la vie de chaque PNJ à partir de la liste globale
        for npc in &world_data.world.npcs {
            self.npc_hps.insert(npc.id.clone(), npc.hp);
        }
        
        println!("[STATE] État de jeu initialisé avec succès depuis le YAML !");
    }

    pub fn add_player(&mut self, addr: SocketAddr, username: String) {
        let new_player = Player {
            username,
            hp: 100,
            exp: 0,
            current_room: "village_square".to_string(),
            inventory: Vec::new(),
            completed_quests: Vec::new(),
            last_move: None,
            last_attack: None,
            last_chat: None,
        };
        self.players.insert(addr, new_player);
    }

    pub fn remove_player(&mut self, addr: SocketAddr) -> Option<Player> {
        self.players.remove(&addr)
    }

    pub fn get_player_mut(&mut self, addr: SocketAddr) -> Option<&mut Player> {
        self.players.get_mut(&addr)
    }
}