use bevy::prelude::*;

use crate::net::ServerMessageEvent;
use crate::AppState;

#[derive(Resource)]
pub struct GameState {
    pub current_room: String,
    pub exits: Vec<String>,
}

impl Default for GameState {
    fn default() -> Self {
        Self { 
            current_room: "unknown".to_string(),
            exits: Vec::new(),
        }
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

        let room_info = room_from_connect(msg).or_else(|| room_from_move(msg));
        if let Some((room, exits)) = room_info {
            if !room.is_empty() && game_state.current_room != room {
                game_state.current_room = room.to_string();
                game_state.exits = exits;
                println!("[GAME] 🗺️ Current room: {} (exits: {:?})", game_state.current_room, game_state.exits);
            }
        }
    }
}

fn extract_exits(msg: &str) -> Vec<String> {
    msg.split_whitespace()
        .find_map(|t| t.strip_prefix("exits="))
        .map(|s| s.split(',').map(|x| x.to_string()).collect())
        .unwrap_or_default()
}

fn room_from_connect(msg: &str) -> Option<(&str, Vec<String>)> {
    let rest = msg.strip_prefix("S: OK connected")?;
    let room = rest.split_whitespace().find_map(|t| t.strip_prefix("room="))?;
    let exits = extract_exits(rest);
    Some((room, exits))
}

fn room_from_move(msg: &str) -> Option<(&str, Vec<String>)> {
    let rest = msg.strip_prefix("S: OK room-loc.")?;
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let room = &rest[..end];
    let exits = extract_exits(&rest[end..]);
    Some((room, exits))
}
