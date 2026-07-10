#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

macro_rules! id_type {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
        impl From<String> for $name {
            fn from(s: String) -> Self {
                $name(s)
            }
        }
        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                $name(s.to_string())
            }
        }
    };
}

id_type!(PlayerId, "Identifiant d'un joueur (son pseudo).");
id_type!(RoomId, "Identifiant d'une salle / d'un lieu.");
id_type!(ItemId, "Identifiant d'un objet (item, arme ou objet-clé).");
id_type!(NpcId, "Identifiant d'un PNJ.");
id_type!(QuestId, "Identifiant d'une quête.");
id_type!(GroupId, "Identifiant d'un groupe de joueurs.");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    pub fn parse(s: &str) -> Option<Direction> {
        match s.to_ascii_lowercase().as_str() {
            "north" => Some(Direction::North),
            "south" => Some(Direction::South),
            "east" => Some(Direction::East),
            "west" => Some(Direction::West),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub damage: i32,
    pub special_damage: i32,
    pub defense: i32,
    pub special_defense: i32,
}

impl Default for Stats {
    fn default() -> Self {
        Stats {
            damage: 0,
            special_damage: 0,
            defense: 0,
            special_defense: 0,
        }
    }
}

pub fn xp_required_for_level(level: i32) -> i32 {
    let n = level.max(1) as f64;
    (4.0 * n.powi(2) + 56.0).round() as i32
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XpBar {
    pub current: i32,
    pub requiered: i32,
}

impl XpBar {
    pub fn new() -> Self {
        XpBar {
            current: 0,
            requiered: xp_required_for_level(1),
        }
    }
}

impl Default for XpBar {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MedallionType {
    Fire,
    Lightning,
    Wind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Medaillon {
    pub kind: MedallionType,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeaponType {
    Magic,
    Melee,
    Range,
}

impl Default for WeaponType {
    fn default() -> Self {
        WeaponType::Melee
    }
}

#[derive(Debug, Clone)]
pub struct Weapon {
    pub id: ItemId,
    pub name: String,
    pub damages: i32,
    pub category: WeaponType,
}

#[derive(Debug, Clone)]
pub struct Armor {
    pub id: ItemId,
    pub name: String,
    pub description: String,
    pub defense: i32,
    pub special_defense: i32,
}

#[derive(Debug, Clone, Default)]
pub struct Equipement {
    pub helmet: Option<Armor>,
    pub chestplate: Option<Armor>,
    pub legging: Option<Armor>,
    pub boot: Option<Armor>,
    pub weapon: Option<Weapon>,
}

impl Equipement {
    pub fn total_defense(&self) -> i32 {
        [&self.helmet, &self.chestplate, &self.legging, &self.boot]
            .iter()
            .filter_map(|slot| slot.as_ref())
            .map(|armor| armor.defense)
            .sum()
    }

    pub fn weapon_damage(&self) -> i32 {
        self.weapon.as_ref().map(|w| w.damages).unwrap_or(0)
    }

    pub fn armor_slot_mut(&mut self, slot: &str) -> Option<&mut Option<Armor>> {
        match slot {
            "head" | "helmet" => Some(&mut self.helmet),
            "chest" | "chestplate" => Some(&mut self.chestplate),
            "legs" | "legging" | "leggings" => Some(&mut self.legging),
            "boot" | "boots" => Some(&mut self.boot),
            _ => None,
        }
    }

    pub fn equipped(&self) -> Vec<(&'static str, ItemId, String)> {
        let mut out = Vec::new();
        if let Some(a) = &self.helmet {
            out.push(("head", a.id.clone(), a.name.clone()));
        }
        if let Some(a) = &self.chestplate {
            out.push(("chest", a.id.clone(), a.name.clone()));
        }
        if let Some(a) = &self.legging {
            out.push(("legs", a.id.clone(), a.name.clone()));
        }
        if let Some(a) = &self.boot {
            out.push(("boot", a.id.clone(), a.name.clone()));
        }
        if let Some(w) = &self.weapon {
            out.push(("weapon", w.id.clone(), w.name.clone()));
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemBucket {
    Item,
    Weapon,
    KeyItem,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Inventory {
    #[serde(default)]
    pub items: HashMap<ItemId, u32>,
    #[serde(default)]
    pub weapons: HashMap<ItemId, u32>,
    #[serde(default)]
    pub key_items: HashMap<ItemId, u32>,
}

impl Inventory {
    fn bucket_mut(&mut self, bucket: ItemBucket) -> &mut HashMap<ItemId, u32> {
        match bucket {
            ItemBucket::Item => &mut self.items,
            ItemBucket::Weapon => &mut self.weapons,
            ItemBucket::KeyItem => &mut self.key_items,
        }
    }

    pub fn add(&mut self, id: ItemId, bucket: ItemBucket) {
        *self.bucket_mut(bucket).entry(id).or_insert(0) += 1;
    }

    pub fn contains(&self, id: &ItemId) -> bool {
        self.items.contains_key(id)
            || self.weapons.contains_key(id)
            || self.key_items.contains_key(id)
    }

    pub fn count(&self, id: &ItemId) -> u32 {
        self.items.get(id).copied().unwrap_or(0)
            + self.weapons.get(id).copied().unwrap_or(0)
            + self.key_items.get(id).copied().unwrap_or(0)
    }

    pub fn remove_one(&mut self, id: &ItemId) -> bool {
        for map in [&mut self.items, &mut self.weapons, &mut self.key_items] {
            if let Some(count) = map.get_mut(id) {
                if *count > 1 {
                    *count -= 1;
                } else {
                    map.remove(id);
                }
                return true;
            }
        }
        false
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.weapons.is_empty() && self.key_items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ItemId, &u32)> {
        self.items
            .iter()
            .chain(self.weapons.iter())
            .chain(self.key_items.iter())
    }
}

#[derive(Debug, Clone)]
pub struct Item {
    pub id: ItemId,
    pub name: String,
    pub description: String,
    pub obtainable: bool,
}

#[derive(Debug, Clone)]
pub struct KeyItem {
    pub id: ItemId,
    pub name: String,
    pub description: String,
    pub quest: Option<QuestId>,
}

#[derive(Debug, Clone)]
pub enum CombatState {
    Idle,
    InCombat { target: NpcId },
    Defeated,
}

impl Default for CombatState {
    fn default() -> Self {
        CombatState::Idle
    }
}

#[derive(Debug, Clone)]
pub enum NpcRole {
    Dialogue,
    QuestGiver { quest: QuestId },
    Enemy { hp: i32, max_hp: i32, stats: Stats },
}

#[derive(Debug, Clone)]
pub struct Group {
    pub id: GroupId,
    pub members: Vec<PlayerId>,
}

#[derive(Debug, Clone)]
pub enum QuestObjective {
    FetchItem { item: ItemId },
    DefeatNpc { npc: NpcId },
    DeliverItem { item: ItemId, to: NpcId },
}

#[derive(Debug, Clone)]
pub struct Reward {
    pub items: Vec<ItemId>,
    pub hp: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestStatus {
    NotStarted,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestProgress {
    pub quest: QuestId,
    pub status: QuestStatus,
}
