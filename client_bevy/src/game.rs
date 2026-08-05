use bevy::prelude::*;

use crate::net::ServerMessageEvent;
use crate::AppState;

#[derive(Resource)]
pub struct GameState {
    pub current_room: String,
}

impl Default for GameState {
    fn default() -> Self {
        Self { current_room: "unknown".to_string() }
    }
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameState>()
            .add_systems(Update, track_current_room);
    }
}

fn track_current_room(mut events: EventReader<ServerMessageEvent>, mut game_state: ResMut<GameState>) {
    for ev in events.read() {
        let msg = &ev.0;

        let room = room_from_connect(msg).or_else(|| room_from_move(msg));
        if let Some(room) = room {
            if !room.is_empty() && game_state.current_room != room {
                game_state.current_room = room.to_string();
                println!("[GAME] 🗺️ Current room: {}", game_state.current_room);
            }
        }
    }
}

fn room_from_connect(msg: &str) -> Option<&str> {
    let rest = msg.strip_prefix("S: OK connected")?;
    rest.split_whitespace().find_map(|t| t.strip_prefix("room="))
}

fn room_from_move(msg: &str) -> Option<&str> {
    let start = msg.find("room-loc.")? + "room-loc.".len();
    let end = msg[start..]
        .find(char::is_whitespace)
        .map_or(msg.len(), |i| start + i);
    Some(&msg[start..end])
}
