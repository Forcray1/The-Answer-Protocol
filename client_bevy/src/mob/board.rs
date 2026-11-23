use bevy::prelude::*;
use crate::mob::components::BoardNode;

/// Spawn/despawn board nodes from `S: EVT ROOM BOARD_SPAWN <id> <x> <y>`.
pub(crate) fn handle_board_spawn_events(
    mut commands: Commands,
    mut events: EventReader<crate::net::ServerMessageEvent>,
    boards: Query<(Entity, &BoardNode)>,
) {
    for ev in events.read() {
        let parts: Vec<&str> = ev.0.split_whitespace().collect();

        // S: EVT ROOM BOARD_SPAWN <npc_id> <x> <y>
        if parts.len() >= 7
            && parts[0] == "S:"
            && parts[1] == "EVT"
            && parts[2] == "ROOM"
            && parts[3] == "BOARD_SPAWN"
        {
            let npc_id = parts[4];
            if boards.iter().any(|(_, b)| b.npc_id == npc_id) {
                continue;
            }
            let x: f32 = parts[5].parse().unwrap_or(0.0);
            let y: f32 = parts[6].parse().unwrap_or(0.0);
            // No sprite — just an invisible transform node for interaction detection.
            commands.spawn((
                TransformBundle::from_transform(Transform::from_xyz(x, y, 0.0)),
                VisibilityBundle::default(),
                BoardNode {
                    npc_id: npc_id.to_string(),
                },
            ));
        }

        // Clean up boards on room change.
        if ev.0.starts_with("S: OK room=loc.") {
            for (entity, _) in &boards {
                commands.entity(entity).despawn_recursive();
            }
        }
    }
}
