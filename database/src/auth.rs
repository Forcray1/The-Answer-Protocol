use argon2::password_hash::{rand_core::OsRng, PasswordHash, SaltString};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use sqlx::SqlitePool;

/// Creates a new player account.
/// Hashes the password with argon2 before writing.
/// Returns false if the username is already taken.
pub async fn register_player(
    pool: &SqlitePool,
    username: &str,
    password: &str,
) -> Result<bool, sqlx::Error> {
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT username FROM players WHERE username = ?")
            .bind(username)
            .fetch_optional(pool)
            .await?;

    if existing.is_some() {
        return Ok(false);
    }

    // Hash the password
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("hash failed")
        .to_string();

    sqlx::query("INSERT INTO players (username, password_hash, current_room) VALUES (?, ?, ?)")
        .bind(username)
        .bind(&hash)
        .bind("Start_oasis")
        .execute(pool)
        .await?;

    Ok(true)
}

/// Verifies an existing player's password.
/// Returns true if the password is correct.
/// Returns false if the player doesn't exist or the password is wrong.
pub async fn verify_player(
    pool: &SqlitePool,
    username: &str,
    password: &str,
) -> Result<bool, sqlx::Error> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT password_hash FROM players WHERE username = ?")
            .bind(username)
            .fetch_optional(pool)
            .await?;

    match row {
        None => Ok(false),
        Some((hash,)) => {
            let parsed = PasswordHash::new(&hash).expect("invalid hash");
            let ok = Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok();
            Ok(ok)
        }
    }
}
