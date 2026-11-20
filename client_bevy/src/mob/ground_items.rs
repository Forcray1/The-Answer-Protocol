use std::time::Duration;
use bevy::prelude::*;
use crate::AppState;
use crate::map::YSort;
use crate::net::ServerMessageEvent;
use crate::player::LocalPlayer;
use crate::player::PlayerName;
use crate::mob::components::{GROUND_ITEM_SIZE, GROUND_ITEM_Z, GroundItem, GroundItemTarget, ITEM_INTERACT_DIST};

pub(crate) fn handle_ground_item_events(
    mut commands: Commands,
    mut events: EventReader<crate::net::ServerMessageEvent>,
    asset_server: Res<AssetServer>,
    existing: Query<(Entity, &GroundItem)>,
    mut ground_target: ResMut<GroundItemTarget>,
) {
    for ev in events.read() {
        let parts: Vec<&str> = ev.0.split_whitespace().collect();

        // S: EVT ROOM ITEM_SPAWN <item_id> <name> <sprite> <x> <y>
        if parts.len() >= 9
            && parts[0] == "S:"
            && parts[1] == "EVT"
            && parts[2] == "ROOM"
            && parts[3] == "ITEM_SPAWN"
        {
            let item_id = parts[4];
            // Skip if already in scene
            if existing.iter().any(|(_, gi)| gi.item_id == item_id) {
                continue;
            }

            let sprite_path = parts[6];
            let x: f32 = parts[7].parse().unwrap_or(0.0);
            let y: f32 = parts[8].parse().unwrap_or(0.0);

            let texture: Handle<Image> = if sprite_path != "none" {
                let normalized = if sprite_path.starts_with("UI/") {
                    sprite_path.to_string()
                } else {
                    format!("UI/{}", sprite_path)
                };
                asset_server.load(format!("{}.png", normalized))
            } else {
                asset_server.load("UI/QuestUI.png")
            };

            commands.spawn((
                SpriteBundle {
                    texture,
                    sprite: Sprite {
                        custom_size: Some(Vec2::splat(GROUND_ITEM_SIZE)),
                        ..default()
                    },
                    transform: Transform::from_xyz(x, y, GROUND_ITEM_Z),
                    ..default()
                },
                GroundItem {
                    item_id: item_id.to_string(),
                },
            ));
        }

        // S: EVT ROOM ITEM_DESPAWN <item_id>
        if parts.len() >= 5
            && parts[0] == "S:"
            && parts[1] == "EVT"
            && parts[2] == "ROOM"
            && parts[3] == "ITEM_DESPAWN"
        {
            let item_id = parts[4];
            for (entity, gi) in &existing {
                if gi.item_id == item_id {
                    commands.entity(entity).despawn_recursive();
                }
            }
            if ground_target.0.as_deref() == Some(item_id) {
                ground_target.0 = None;
            }
        }
    }
}

/// Separate system that runs after handle_ground_item_events to find the closest ground item.
pub(crate) fn update_ground_item_target_impl(
    player_q: Query<&Transform, With<crate::player::LocalPlayer>>,
    items_q: Query<(&Transform, &GroundItem)>,
    mut ground_target: ResMut<GroundItemTarget>,
    combat: Res<crate::combat::CombatResource>,
    dialogue: Res<crate::interaction::DialogueState>,
) {
    if combat.in_combat || dialogue.is_active() {
        ground_target.0 = None;
        return;
    }
    let Ok(player_tf) = player_q.get_single() else {
        return;
    };
    let player_xy = player_tf.translation.truncate(); // ignore z

    let mut closest_dist = ITEM_INTERACT_DIST;
    let mut closest_id: Option<String> = None;

    for (tf, gi) in &items_q {
        // Use 2-D distance so the item's render-layer z (400) does not inflate the gap.
        let dist = player_xy.distance(tf.translation.truncate());
        if dist < closest_dist {
            closest_dist = dist;
            closest_id = Some(gi.item_id.clone());
        }
    }
    if ground_target.0 != closest_id {
        ground_target.0 = closest_id;
    }
}

/// Clear all ground items when entering a new room.
pub(crate) fn cleanup_items_on_room_change(
    mut commands: Commands,
    mut events: EventReader<crate::net::ServerMessageEvent>,
    items: Query<Entity, With<GroundItem>>,
    mut ground_target: ResMut<GroundItemTarget>,
) {
    for ev in events.read() {
        if ev.0.starts_with("S: OK room=loc.") {
            for entity in &items {
                commands.entity(entity).despawn_recursive();
            }
            ground_target.0 = None;
        }
    }
}
