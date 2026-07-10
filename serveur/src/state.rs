// serveur/src/state.rs
//
// L'état de jeu VIVANT, en RAM (le "placement" : qui est où, quels objets au sol,
// les PV courants). Le `Player` runtime a été enrichi pour épouser le modèle de
// `domain` (adapté de classes.rs) : niveau, argent, barre d'XP, stats, équipement
// structuré, inventaire à trois sacs, état de combat…

use std::collections::HashMap;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::Duration;
use std::time::Instant;

use crate::world::WorldData;
use domain::{
    xp_required_for_level, CombatState, Equipement, GroupId, Inventory, ItemId, Medaillon, NpcId,
    PlayerId, RoomId, Stats, XpBar,
};

/// Salle de départ d'un nouveau joueur (et point de réapparition à la mort).
pub const START_ROOM: &str = "Start_oasis";

#[derive(Debug, Clone, PartialEq)]
pub enum ItemSource {
    World,
    MobDrop,
    PlayerDrop,
}

#[derive(Debug, Clone)]
pub struct RuntimeItem {
    pub item_id: ItemId,
    pub source: ItemSource,
    pub collected_by: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CombatPhase {
    WaitingForPlayerAction,
    WaitingForPlayerQte {
        qte_type: String,
        started_at: Instant,
        duration: Duration,
    },
    WaitingForNpcRiposte {
        started_at: Instant,
        duration: Duration,
    },
}

#[derive(Debug, Clone)]
pub struct CombatInstance {
    pub npc_id: NpcId,
    pub phase: CombatPhase,
    pub turn_number: u32,
}

/// Le joueur tel qu'il vit en mémoire pendant une session.
///
/// Les champs persistés (hp, xp, niveau, argent, salle, inventaire, équipement,
/// quêtes) sont sauvegardés/rechargés via la crate `database`. Les champs
/// purement runtime (`combat`, `group`, les horodatages anti-spam) ne le sont pas.
#[derive(Debug, Clone)]
pub struct Player {
    pub id: PlayerId,
    pub username: String,
    pub skin: String,
    pub pos_x: f32,
    pub pos_y: f32,
    pub hp: i32,
    pub max_hp: i32,
    pub level: i32,
    pub xp_bar: XpBar,
    pub money: i32,
    pub stats: Stats,
    pub medaillon: Option<Medaillon>,
    pub current_room: RoomId,
    pub inventory: Inventory,
    pub equipement: Equipement,
    pub completed_quests: Vec<domain::QuestId>,
    pub group: Option<GroupId>,
    pub combat: CombatState,
    pub last_move: Option<Instant>,
    pub last_attack: Option<Instant>,
    pub last_chat: Option<Instant>,
}

impl Player {
    /// Crée un joueur frais (nouveau compte) dans la salle de départ.
    pub fn new(username: String, room: RoomId) -> Self {
        Player {
            id: PlayerId(username.clone()),
            username,
            skin: "default".to_string(),
            pos_x: 0.0,
            pos_y: 0.0,
            hp: 100,
            max_hp: 100,
            level: 1,
            xp_bar: XpBar::new(),
            money: 0,
            stats: Stats::default(),
            medaillon: None,
            current_room: room,
            inventory: Inventory::default(),
            equipement: Equipement::default(),
            completed_quests: Vec::new(),
            group: None,
            combat: CombatState::Idle,
            last_move: None,
            last_attack: None,
            last_chat: None,
        }
    }

    /// Ajoute de l'XP et fait monter de niveau autant de fois que nécessaire.
    /// Renvoie le nombre de niveaux gagnés. (Version corrigée de classes.rs, qui
    /// confondait `self.current`/`levels_gained` et ne compilait pas.)
    pub fn add_xp(&mut self, amount: i32) -> i32 {
        self.xp_bar.current += amount;
        let mut levels_gained = 0;
        while self.xp_bar.current >= self.xp_bar.requiered {
            self.xp_bar.current -= self.xp_bar.requiered;
            self.level += 1;
            levels_gained += 1;
            self.xp_bar.requiered = xp_required_for_level(self.level);
        }
        levels_gained
    }

    pub fn add_money(&mut self, amount: i32) {
        self.money += amount;
    }

    pub fn remove_money(&mut self, amount: i32) -> bool {
        if self.money >= amount {
            self.money -= amount;
            true
        } else {
            false
        }
    }
}

pub struct DeadNpc {
    pub npc_id: NpcId,
    pub room_id: RoomId,
    pub respawn_at: Instant,
}

pub struct ServerState {
    pub players: HashMap<SocketAddr, Player>,
    pub room_items: HashMap<RoomId, Vec<RuntimeItem>>,
    pub room_npcs: HashMap<RoomId, Vec<NpcId>>,
    pub npc_hps: HashMap<NpcId, i32>,
    pub active_combats: HashMap<SocketAddr, CombatInstance>,
    pub dead_npcs: Vec<DeadNpc>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            room_items: HashMap::new(),
            room_npcs: HashMap::new(),
            npc_hps: HashMap::new(),
            active_combats: HashMap::new(),
            dead_npcs: Vec::new(),
        }
    }

    pub fn update_respawns(&mut self, world_data: &WorldData) {
        let now = Instant::now();
        let mut to_respawn = Vec::new();

        self.dead_npcs.retain(|dead| {
            if now >= dead.respawn_at {
                to_respawn.push((dead.npc_id.clone(), dead.room_id.clone()));
                false
            } else {
                true
            }
        });

        for (npc_id, room_id) in to_respawn {
            if let Some(npc) = world_data.world.npcs.iter().find(|n| n.id == npc_id) {
                self.npc_hps.insert(npc_id.clone(), npc.hp);
                self.room_npcs.entry(room_id).or_default().push(npc_id);
            }
        }
    }

    pub fn initialize_from_world(&mut self, world_data: &WorldData) {
        for (room_id, location) in &world_data.world.locations {
            let runtime_items: Vec<RuntimeItem> = location
                .items
                .iter()
                .map(|id| RuntimeItem {
                    item_id: id.clone(),
                    source: ItemSource::World,
                    collected_by: HashSet::new(),
                })
                .collect();

            self.room_items.insert(room_id.clone(), runtime_items);
            self.room_npcs.insert(room_id.clone(), location.npcs.clone());
        }

        for npc in &world_data.world.npcs {
            self.npc_hps.insert(npc.id.clone(), npc.hp);
        }

        println!("[STATE] Game state successfully initialized from YAML!");
    }

    pub fn add_player(&mut self, addr: SocketAddr, username: String) {
        let new_player = Player::new(username, RoomId::from(START_ROOM));
        self.players.insert(addr, new_player);
    }

    pub fn remove_player(&mut self, addr: SocketAddr) -> Option<Player> {
        self.players.remove(&addr)
    }

    pub fn get_player_mut(&mut self, addr: SocketAddr) -> Option<&mut Player> {
        self.players.get_mut(&addr)
    }
}
