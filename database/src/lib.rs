// - init_db   : creates game.db and tables if they don't exist
// - register_player : creates a player account
// - verify_player  : checks password on login
// - load_player    : loads persisted player data
// - save_player    : saves player data
//
// Since adopting the `domain` model, persistence uses strong types
// (Inventory, ItemId, QuestId, RoomId) instead of raw Strings/HashMaps.
// JSON columns now store: the full inventory (3 bags) as one
// object, equipment as slot -> id map, and quests as id list.

use std::collections::HashMap;
use std::str::FromStr;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions, sqlite::SqliteConnectOptions};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{rand_core::OsRng, SaltString, PasswordHash};

use domain::{Inventory, ItemId, QuestId, RoomId};

pub struct PlayerData {
    pub hp: i32,
    pub exp: i32,
    pub level: i32,
    pub money: i32,
    pub current_room: RoomId,
    pub inventory: Inventory,
    /// slot ("head", "weapon"…) -> id of the equipped item.
    pub equipment: HashMap<String, ItemId>,
    pub completed_quests: Vec<QuestId>,
    /// Skin name (file under sprites/skin/, no extension). "default" by default.
    pub skin: String,
    /// Last position (x, y) in the room. `None` if never saved
    /// (new player): client falls back to its spawn point.
    pub position: Option<(f32, f32)>,
}

// Init

// Creates the SQLite connection pool and runs migrations.
// The game.db file is created if it doesn't exist.

pub async fn init_db(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    // create_if_missing: creates game.db if it doesn't exist yet
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    // Runs all migration files in order
    // Stops cleanly if migrations have already been applied
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

// Creates a new player account.
// Hashes the password with argon2 before writing.
// Returns false if the username is already taken.

pub async fn register_player(
    pool: &SqlitePool,
    username: &str,
    password: &str,
) -> Result<bool, sqlx::Error> {
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT username FROM players WHERE username = ?"
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    if existing.is_some() {
        return Ok(false); // Username already taken
    }

    // Hash the password
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("hash failed")
        .to_string();

    sqlx::query(
        "INSERT INTO players (username, password_hash) VALUES (?, ?)"
    )
    .bind(username)
    .bind(&hash)
    .execute(pool)
    .await?;

    Ok(true)
}

// Verifies an existing player's password.
// Returns true if the password is correct.
// Returns false if the player doesn't exist or the password is wrong.

pub async fn verify_player(
    pool: &SqlitePool,
    username: &str,
    password: &str,
) -> Result<bool, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT password_hash FROM players WHERE username = ?"
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    match row {
        None => Ok(false), // Player not found
        Some((hash,)) => {
            let parsed = PasswordHash::new(&hash).expect("invalid hash");
            let ok = Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok();
            Ok(ok)
        }
    }
}

// Load player save

// Loads persisted player data.
// Returns None if the player doesn't exist.

pub async fn load_player(
    pool: &SqlitePool,
    username: &str,
) -> Result<Option<PlayerData>, sqlx::Error> {
    let row: Option<(i32, i32, i32, i32, String, String, String, String, String, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT hp, exp, level, money, current_room, inventory, equipment, completed_quests, skin, pos_x, pos_y
         FROM players WHERE username = ?"
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    match row {
        None => Ok(None),
        Some((hp, exp, level, money, current_room, inv_json, equip_json, quests_json, skin, pos_x, pos_y)) => {
            // Colonnes JSON vers types Rust forts
            let inventory: Inventory =
                serde_json::from_str(&inv_json).unwrap_or_default();
            let equipment: HashMap<String, ItemId> =
                serde_json::from_str(&equip_json).unwrap_or_default();
            let completed_quests: Vec<QuestId> =
                serde_json::from_str(&quests_json).unwrap_or_default();

            // Position only if both columns are set.
            let position = match (pos_x, pos_y) {
                (Some(x), Some(y)) => Some((x as f32, y as f32)),
                _ => None,
            };

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
            }))
        }
    }
}

// Saves the current player state.
// Call when the player disconnects.

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
) -> Result<(), sqlx::Error> {
    let inv_json = serde_json::to_string(inventory).unwrap_or_default();
    let equip_json = serde_json::to_string(equipment).unwrap_or_default();
    let quests_json = serde_json::to_string(completed_quests).unwrap_or_default();

    sqlx::query(
        "UPDATE players
         SET hp = ?, exp = ?, level = ?, money = ?, current_room = ?,
             inventory = ?, equipment = ?, completed_quests = ?, skin = ?,
             pos_x = ?, pos_y = ?
         WHERE username = ?"
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
    .bind(username)
    .execute(pool)
    .await?;

    Ok(())
}
