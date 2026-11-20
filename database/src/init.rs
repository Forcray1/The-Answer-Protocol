use std::str::FromStr;

use sqlx::{sqlite::SqliteConnectOptions, sqlite::SqlitePoolOptions, SqlitePool};

/// Creates the SQLite connection pool and runs the migrations.
/// The game.db file is created if it doesn't exist.
pub async fn init_db(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    // create_if_missing: creates game.db if it doesn't exist yet
    let options = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    // Runs all migration files in order
    // Stops cleanly if migrations have already been applied
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}
