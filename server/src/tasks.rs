//! Background tasks started by `main`: NPC respawns and clean shutdown.

use std::sync::Arc;
use std::time::Duration;

use domain::RoomId;
use tokio::sync::broadcast::Sender;
use tokio::sync::Mutex;

use crate::events::{to_room, GlobalEvent};
use crate::handlers::{npc_spawn_event, save_player_to_db};
use crate::persistence::save_world_state;
use crate::state::ServerState;
use crate::world::WorldData;
use crate::DbPool;

/// Respawn loop: independent timer so NPC respawns do not depend on player commands.
pub fn spawn_respawn_loop(
    world: Arc<WorldData>,
    state: Arc<Mutex<ServerState>>,
    tx: Arc<Sender<GlobalEvent>>,
) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        loop {
            ticker.tick().await;
            let mut events_to_emit: Vec<(RoomId, String)> = Vec::new();
            {
                let mut guard = state.lock().await;
                let respawned = guard.update_respawns(&world);
                for (npc_id, room_id) in respawned {
                    if let Some(npc) = world.npc_by_id(&npc_id) {
                        let hp = guard.npc_hps.get(&npc_id).copied().unwrap_or(npc.hp);
                        events_to_emit.push((room_id.clone(), npc_spawn_event(npc, hp)));
                    }
                }
            }
            for (room_id, message) in events_to_emit {
                to_room(&tx, None, message, room_id);
            }
        }
    });
}

/// Waits for SIGINT (Ctrl-C), saves players and ground items, then exits.
pub fn spawn_shutdown_handler(state: Arc<Mutex<ServerState>>, pool: DbPool) {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            println!("[SERVER] Shutdown signal received. Saving player data and world state…");
            let (players_to_save, room_items) = {
                let guard = state.lock().await;
                let players = guard.players.values().cloned().collect::<Vec<_>>();
                let room_items = guard.room_items.clone();
                (players, room_items)
            };
            for player in &players_to_save {
                save_player_to_db(&pool, player).await;
            }
            save_world_state(&room_items, "world.yaml");
            println!("[SERVER] All data saved. Goodbye.");
            std::process::exit(0);
        }
    });
}
