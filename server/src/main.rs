use std::collections::{HashMap, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex};

pub mod board_quests;
mod commands;
mod connection;
mod events;
pub mod handlers;
mod logging;
mod persistence;
mod state;
mod tasks;
mod world;

use events::GlobalEvent;
use state::ServerState;
use world::WorldData;

// The SQLite pool is shared across handlers through Arc.
// SqlitePool is already thread-safe, so no Mutex is needed around it.
pub type DbPool = Arc<sqlx::SqlitePool>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|arg| arg == "--logs") {
        logging::LOG_MODE.store(true, Ordering::SeqCst);
        println!("[SERVER] JSON log mode enabled.");
    }

    // 1. Initialize the database — creates game.db + tables when needed.
    println!("[SERVER] Initializing database...");
    let pool = database::init_db("sqlite://game.db").await?;
    let shared_pool: DbPool = Arc::new(pool);
    println!("[SERVER] Database ready.");

    // 2. Load the world from YAML.
    println!("[SERVER] Loading map...");
    let world_data = match WorldData::load_from_file("world.yaml") {
        Ok(data) => data,
        Err(e) => {
            eprintln!("[SERVER] Failed to load world.yaml: {}", e);
            return Err(e);
        }
    };
    let shared_world = Arc::new(world_data);

    // 3. Initialize in-memory game state.
    let mut initial_state = ServerState::new();
    initial_state.initialize_from_world(&shared_world);
    let shared_state = Arc::new(Mutex::new(initial_state));

    let (tx, _rx) = broadcast::channel::<GlobalEvent>(32);
    let shared_tx = Arc::new(tx);

    tasks::spawn_respawn_loop(
        Arc::clone(&shared_world),
        Arc::clone(&shared_state),
        Arc::clone(&shared_tx),
    );

    let listener = TcpListener::bind("127.0.0.1:4243").await?;
    println!("[SERVER] Listening on 4243...");

    tasks::spawn_shutdown_handler(Arc::clone(&shared_state), Arc::clone(&shared_pool));

    let mut recent_connections: HashMap<String, VecDeque<Instant>> = HashMap::new();
    loop {
        let (socket, addr) = listener.accept().await?;
        connection::note_connection(&mut recent_connections, &addr.ip().to_string(), Instant::now());

        tokio::spawn(connection::handle_client(
            socket,
            addr,
            Arc::clone(&shared_state),
            Arc::clone(&shared_world),
            Arc::clone(&shared_tx),
            Arc::clone(&shared_pool), // clone of the Arc, not the pool itself
            shared_tx.subscribe(),
        ));
    }
}
