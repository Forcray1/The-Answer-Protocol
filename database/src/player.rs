use std::collections::HashMap;

use domain::{Inventory, ItemId, QuestId, RoomId};
use sqlx::SqlitePool;

use crate::model::PlayerData;

type PlayerRow = (
    i32,
    i32,
    i32,
    i32,
    String,
    String,
    String,
    String,
    String,
    Option<f64>,
    Option<f64>,
    i32,
);

/// Loads persisted player data.
/// Returns None if the player doesn't exist.
pub async fn load_player(
    pool: &SqlitePool,
    username: &str,
) -> Result<Option<PlayerData>, sqlx::Error> {
    let row: Option<PlayerRow> = sqlx::query_as(
        "SELECT hp, exp, level, money, current_room, inventory, equipment, completed_quests, skin, pos_x, pos_y, stat_points
         FROM players WHERE username = ?"
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    // Load base stats separately to avoid tuple-size limits in sqlx.
    let stats_row: Option<(i32, i32, i32, i32, i32)> = sqlx::query_as(
        "SELECT base_damage, base_defense, base_spe_dmg, base_spe_def, base_max_hp
         FROM players WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    match row {
        None => Ok(None),
        Some((
            hp,
            exp,
            level,
            money,
            current_room,
            inv_json,
            equip_json,
            quests_json,
            skin,
            pos_x,
            pos_y,
            stat_points,
        )) => {
            let inventory: Inventory = serde_json::from_str(&inv_json).unwrap_or_default();
            let equipment: HashMap<String, ItemId> =
                serde_json::from_str(&equip_json).unwrap_or_default();
            let completed_quests: Vec<QuestId> =
                serde_json::from_str(&quests_json).unwrap_or_default();

            let position = match (pos_x, pos_y) {
                (Some(x), Some(y)) => Some((x as f32, y as f32)),
                _ => None,
            };

            let (base_damage, base_defense, base_spe_dmg, base_spe_def, base_max_hp) =
                stats_row.unwrap_or((0, 0, 0, 0, 0));

            Ok(Some(PlayerData {
                hp,
                exp,
                level,
                money,
                current_room: RoomId(current_room),
                inventory,
                equipment,
                completed_quests,
                skin,
                position,
                stat_points,
                base_damage,
                base_defense,
                base_spe_dmg,
                base_spe_def,
                base_max_hp,
            }))
        }
    }
}

/// Saves the current player state.
/// Call when the player disconnects.
#[allow(clippy::too_many_arguments)]
pub async fn save_player(
    pool: &SqlitePool,
    username: &str,
    hp: i32,
    exp: i32,
    level: i32,
    money: i32,
    current_room: &str,
    inventory: &Inventory,
    equipment: &HashMap<String, ItemId>,
    completed_quests: &Vec<QuestId>,
    skin: &str,
    pos_x: f64,
    pos_y: f64,
    stat_points: i32,
    base_damage: i32,
    base_defense: i32,
    base_spe_dmg: i32,
    base_spe_def: i32,
    base_max_hp: i32,
) -> Result<(), sqlx::Error> {
    let inv_json = serde_json::to_string(inventory).unwrap_or_default();
    let equip_json = serde_json::to_string(equipment).unwrap_or_default();
    let quests_json = serde_json::to_string(completed_quests).unwrap_or_default();

    sqlx::query(
        "UPDATE players
         SET hp = ?, exp = ?, level = ?, money = ?, current_room = ?,
             inventory = ?, equipment = ?, completed_quests = ?, skin = ?,
             pos_x = ?, pos_y = ?, stat_points = ?,
             base_damage = ?, base_defense = ?, base_spe_dmg = ?, base_spe_def = ?, base_max_hp = ?
         WHERE username = ?",
    )
    .bind(hp)
    .bind(exp)
    .bind(level)
    .bind(money)
    .bind(current_room)
    .bind(&inv_json)
    .bind(&equip_json)
    .bind(&quests_json)
    .bind(skin)
    .bind(pos_x)
    .bind(pos_y)
    .bind(stat_points)
    .bind(base_damage)
    .bind(base_defense)
    .bind(base_spe_dmg)
    .bind(base_spe_def)
    .bind(base_max_hp)
    .bind(username)
    .execute(pool)
    .await?;

    Ok(())
}
