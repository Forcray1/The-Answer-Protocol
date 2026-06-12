// - init_db : crée game.db et les tables si elles n'existent pas
// - register_player : crée un compte
// - verify_player : vérifie le mot de passe à la connexion
// - load_player : charge les données persistées d'un joueur
// - save_player : sauvegarde les données d'un joueur

use std::collections::HashMap;
use std::str::FromStr;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions, sqlite::SqliteConnectOptions};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{rand_core::OsRng, SaltString, PasswordHash};

pub struct PlayerData {
    pub hp: i32,
    pub exp: i32,
    pub current_room: String,
    pub inventory: HashMap<String, u32>,
    pub equipment: HashMap<String, String>,
    pub completed_quests: Vec<String>,
}

// Init

// Crée le pool de connexions SQLite et exécute les migrations.
// Le fichier game.db est créé s'il n'existe pas.

pub async fn init_db(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    // create_if_missing : crée le fichier game.db s'il n'existe pas encore
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    // Exécute tous les fichiers dans migrations dans l'ordre
    // S'arrête proprement si les migrations ont déjà été jouées
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

// Crée un nouveau compte joueur.
// Hache le mot de passe avec argon2 avant de l'écrire.
// Retourne false si le pseudo est déjà pris.

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
        return Ok(false); // Pseudo déjà pris
    }

    // Hache le mot de passe
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

// Vérifie le mot de passe d'un joueur existant.
// Retourne true si le mot de passe est correct.
// Retourne false si le joueur n'existe pas ou si le mot de passe est faux.

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
        None => Ok(false), // Joueur pas dans données
        Some((hash,)) => {
            let parsed = PasswordHash::new(&hash).expect("hash invalide");
            let ok = Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok();
            Ok(ok)
        }
    }
}

//load la save

// Charge les données persistées d'un joueur.
// Retourne None si le joueur n'existe pas.

pub async fn load_player(
    pool: &SqlitePool,
    username: &str,
) -> Result<Option<PlayerData>, sqlx::Error> {
    let row: Option<(i32, i32, String, String, String, String)> = sqlx::query_as(
        "SELECT hp, exp, current_room, inventory, equipment, completed_quests
         FROM players WHERE username = ?"
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    match row {
        None => Ok(None),
        Some((hp, exp, current_room, inv_json, equip_json, quests_json)) => {
            // Colonnes JSON vers types Rust
            let inventory: HashMap<String, u32> =
                serde_json::from_str(&inv_json).unwrap_or_default();
            let equipment: HashMap<String, String> =
                serde_json::from_str(&equip_json).unwrap_or_default();
            let completed_quests: Vec<String> =
                serde_json::from_str(&quests_json).unwrap_or_default();

            Ok(Some(PlayerData {
                hp,
                exp,
                current_room,
                inventory,
                equipment,
                completed_quests,
            }))
        }
    }
}

// Save l'état actuel du joueur.
// Appeler quand le joueur se déconnecte.

pub async fn save_player(
    pool: &SqlitePool,
    username: &str,
    hp: i32,
    exp: i32,
    current_room: &str,
    inventory: &HashMap<String, u32>,
    equipment: &HashMap<String, String>,
    completed_quests: &Vec<String>,
) -> Result<(), sqlx::Error> {
    let inv_json = serde_json::to_string(inventory).unwrap_or_default();
    let equip_json = serde_json::to_string(equipment).unwrap_or_default();
    let quests_json = serde_json::to_string(completed_quests).unwrap_or_default();

    sqlx::query(
        "UPDATE players
         SET hp = ?, exp = ?, current_room = ?,
             inventory = ?, equipment = ?, completed_quests = ?
         WHERE username = ?"
    )
    .bind(hp)
    .bind(exp)
    .bind(current_room)
    .bind(&inv_json)
    .bind(&equip_json)
    .bind(&quests_json)
    .bind(username)
    .execute(pool)
    .await?;

    Ok(())
}