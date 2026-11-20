use std::collections::HashMap;

use domain::{Inventory, ItemId, QuestId, RoomId};

pub struct PlayerData {
    pub hp: i32,
    pub exp: i32,
    pub level: i32,
    pub money: i32,
    pub current_room: RoomId,
    pub inventory: Inventory,
    /// slot ("chest", "weapon"…) -> id of the equipped item.
    pub equipment: HashMap<String, ItemId>,
    pub completed_quests: Vec<QuestId>,
    pub skin: String,
    /// Last position (x, y) in the room. `None` if never saved
    /// (new player): client falls back to its spawn point.
    pub position: Option<(f32, f32)>,
    /// Unspent stat allocation points.
    pub stat_points: i32,
    /// Base stats allocated via STATUP (not counting equipment bonuses).
    pub base_damage: i32,
    pub base_defense: i32,
    pub base_spe_dmg: i32,
    pub base_spe_def: i32,
    pub base_max_hp: i32,
}
