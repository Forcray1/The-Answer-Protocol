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
pub enum MedallionType {
	Fire,
	Lightning,
	Wind
}
#[derive(Debug, Clone)]
pub struct Medaillon {
	pub kind: MedallionType,
	pub name: String,
	pub description: String,
}

#[derive(Debug, Clone)]
pub struct XpBar {
	pub current: i32,
	pub requiered: i32,
}

impl XpBar {
	pub fn init() -> Self{
		XpBar {
			current: 0,
			requiered: xp_requiered_for_level(1),
		}
	}
}

impl Default for XpBar {
	fn default() -> Self {
		Self::new()
	}
}

pub struct Equipement {
	pub helmet: Option<Armor>,
	pub chestplate: Option<Armor>,
	pub legging: Option<Armor>,
	pub boot: Option<Armor>,
	pub weapon: Option<Weapon>,
}

pub struct Armor {
	pub id: ItemId,
	pub name: String,
	pub description: String,
	pub defense: i32,
	pub special_defense; i32
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
	pub equipement: Equipement,
	pub medaillon: Option<Medaillon>,
	pub hp: i32,
	pub max_hp: i32,
	pub level: i32,
	pub xp_bar: XpBar,
	pub money: i32,
	pub stats: Stats,
	pub location: RoomId,
	pub quests: Vec<QuestProgress>,
	pub group: Option<GroupId>,
	pub combat: CombatState,
}

impl Player {
	pub fn add_xp(&mut self, amount: i32) -> i32 {
		self.current += amount;
		let mut level_gain = 0;

		while self.xp_bar.current >= self.xp_bar.requiered {
			self.xp_bar.current -= self.xp_bar.requiered;
			self.level += 1;
			levels_gained += 1;
			self.xp_bar.requiered = xp_requiered_for_level(self.level);
		}
	}

	pub fn add_money(&mut self, amount: i32) {
		self.money += amount
	}

	pub fn remove_money(&mut self, amount: i32) -> bool {
		if self.money >= amount {
			self.money -= amount;
			true
		} else {
			false
		}
	}

	fn pub gain_xp(&mut self, amount: i32) -> i32 {
		self.xp_bar.add_xp(amount, &mut self.level)
	}

	pub fn equip_armor(&mut self, item_id: &ItemId, slot: &str, armor:Armor) -> bool {
		if !self.inventory.items.contains_key(item_id){
			return false;
		}

		let count = self.inventory.items.get_mut(item_id).unwrap();
		if *count > 1 {
			*count -= 1;
		} else {
			self.inventory.remove(item_id);
		}

		let prev_eqquiped = match slot (
			"helmet" => self.equipement.helmet.take(),
			"chestplate" => self.equipement.chestplate.take(),
			"legging" => self.equipement.legging.take(),
			"boot" => self.equipement.boot.take(),
			_ => return false,
		);
		if let Some(old) = prev_eqquiped {
			*self.inventory.items.entry(old.id).or_insert(0) += 1;
		}

		match slot {
			"helmet" => self.equipement.helmet = Some(armor),
			"chestplate" => self.equipement.chestplate = Some(armor),
			"legging" => self.equipement.legging = Some(armor),
			"boot" => self.equipement.boot = Some(armor),
			_ => {},
		}
		true
	}

	pub fn equip_weapon(&mut self, item_id: &ItemId, weapon: Weapon) -> bool {
		if !self.inventory.weapons.contains_key(item_id) {
			return false;
		}

		let count = self.inventory.weapons.get_mut(item_id).unwrap();
		if *count > 1 {
			*count -= 1;
		} else {
			self.inventory.remove(item_id);
		}

		if let Some(old) = self.equipement.weapon.take() {
			*self.inventory.weapons.entry(old.id).or_insert(0) += 1;
		}
		self.equipement.weapon = Some(weapon);
		true
	}

	pub fn unequip(&mut self, slot: &str) -> bool {
		let removed = match slot {
			"helmet" => self.equipement.helmet.take().map(|a| (a.id, "armor")),
			"chestplate" => self.equipement.chestplate.take().map(|a| (a.id, "armor")),
			"legging" => self.equipement.legging.take().map(|a| (a.id, "armor")),
			"boot" => self.equipement.boot.take().map(|a| (a.id, "armor")),
			"weapon" => self.equipement.weapon.take().map(|a| (a.id, "armor")),
			_ => return false,
		};

		match removed ={
			None => false,
			Some((id, "weapon")) => {
				*self.inventory.weapons.entry(id).or_insert(0) += 1;
				true
			}
			Some((id, _)) => {
				*self.inventory.items.entry(id).or_insert(0) += 1;
				true
			}
		}
	}
}

pub fn xp_requiered_for_level(level: i32) -> i32 {
	let base: f64 = 60.0;
	let growth: f64 = 1.8;
	let n = level.max(1) as f64;

	(base * n.powf(growth)).round() as i32
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
