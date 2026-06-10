use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlayerId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoomId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NpcId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QuestId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GroupId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
	North,
	South,
	East,
	West,
}

#[derive(Debug, Clone)]
pub struct Stats {
	pub damage: i32,
	pub special_damage: i32,
	pub defense: i32,
	pub special_defense: i32,
}

#[derive(Debug, Clone)]
pub enum CombatState {
	Idle,
	InCombat { target: NpcId },
	Defeated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WeaponType {
	Magic,
	Melee,
	Range,
}

#[derive(Debug, Clone)]
pub struct Weapon {
	pub id: ItemId,
	pub name: String,
	pub damages: i32,
	pub category: WeaponType,
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

#[derive(Debug, Clone, Default)]
pub struct Inventory {
	pub items: HashMap<ItemId, u32>,
	pub weapons: HashMap<ItemId, u32>,
	pub key_items: HashMap<ItemId, u32>,
}

pub struct Player {
	pub id: PlayerId,
	pub name: String,
	pub password: String,
	pub inventory: Inventory,
	pub weapon: Option<Weapon>,
	pub hp: i32,
	pub max_hp: i32,
	pub stats: Stats,
	pub location: RoomId,
	pub quests: Vec<QuestProgress>,
	pub group: Option<GroupId>,
	pub combat: CombatState,
}

#[derive(Debug, Clone)]
pub struct Group {
	pub id: GroupId,
	pub members: Vec<PlayerId>,
}

#[derive(Debug, Clone)]
pub enum NpcRole {
	Dialogue,
	QuestGiver { quest: QuestId },
	Enemy { hp: i32, max_hp: i32, stats: Stats },
}

#[derive(Debug, Clone)]
pub struct Npc {
	pub id: NpcId,
	pub name: String,
	pub description: String,
	pub dialogue: Vec<String>,
	pub role: NpcRole,
}

#[derive(Debug, Clone)]
pub struct Room {
	pub id: RoomId,
	pub name: String,
	pub description: String,
	pub exits: HashMap<Direction, RoomId>,
	pub items: Vec<ItemId>,
	pub npcs: Vec<NpcId>,
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

#[derive(Debug, Clone)]
pub struct Quest {
	pub id: QuestId,
	pub name: String,
	pub description: String,
	pub objective: QuestObjective,
	pub reward: Reward,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestStatus {
	NotStarted,
	InProgress,
	Completed,
}

#[derive(Debug, Clone)]
pub struct QuestProgress {
	pub quest: QuestId,
	pub status: QuestStatus,
}

pub struct World {
	pub rooms: HashMap<RoomId, Room>,
	pub items: HashMap<ItemId, Item>,
	pub key_items: HashMap<ItemId, KeyItem>,
	pub weapons: HashMap<ItemId, Weapon>,
	pub npcs: HashMap<NpcId, Npc>,
	pub quests: HashMap<QuestId, Quest>,
	pub players: HashMap<PlayerId, Player>,
	pub groups: HashMap<GroupId, Group>,
}
