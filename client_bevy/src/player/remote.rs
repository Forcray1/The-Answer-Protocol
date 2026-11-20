use std::time::Duration;
use bevy::prelude::*;
use crate::AppState;
use crate::loading::{RoomTransition, RoomTransitionRequest};
use crate::map::YSort;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::animation::animate;
use crate::player::components::{LocalPlayer, LocalPlayerName, PendingDeathRespawn, PlayerAnimation, RemotePlayer};
use crate::player::config::{DEATH_RESPAWN_POINT, PLAYER_SPEED};
use crate::player::spawn::spawn_player_visual;

pub(crate) fn handle_presence_events(
    mut commands: Commands,
    mut events: EventReader<ServerMessageEvent>,
    asset_server: Res<AssetServer>,
    local_name: Res<LocalPlayerName>,
    mut pending_death: ResMut<PendingDeathRespawn>,
    mut remotes: Query<(Entity, &mut RemotePlayer)>,
    mut local_query: Query<
        (&mut Transform, &mut PlayerAnimation, &mut Handle<Image>),
        With<LocalPlayer>,
    >,
) {
    for ev in events.read() {
        // Detect death: server sends "S: EVT COMBAT END defeat" before the room switch.
        if ev.0.contains("COMBAT END defeat") {
            pending_death.0 = true;
            continue;
        }

        if ev.0.starts_with("S: OK room=loc.") {
            for (entity, _) in &remotes {
                commands.entity(entity).despawn_recursive();
            }

            // The dying player never receives the PRESENCE ENTER GlobalEvent
            // (broadcaster filters it because sender_addr == addr).
            // The only position-related event they receive is this room=loc line,
            // so apply the death snap here and consume the flag.
            if pending_death.0 {
                let room_name = ev
                    .0
                    .strip_prefix("S: OK room=loc.")
                    .and_then(|r| r.split_whitespace().next())
                    .unwrap_or_default();
                if room_name == "Start_oasis" {
                    pending_death.0 = false;
                    if let Ok((mut transform, mut anim, mut texture)) =
                        local_query.get_single_mut()
                    {
                        transform.translation.x = DEATH_RESPAWN_POINT.x;
                        transform.translation.y = DEATH_RESPAWN_POINT.y;
                        animate(
                            &mut anim,
                            &mut texture,
                            Duration::from_millis(0),
                            false,
                            Vec2::ZERO,
                        );
                    }
                }
            }
            continue;
        }

        let p: Vec<&str> = ev.0.split_whitespace().collect();
        if p.len() < 4 || p[0] != "S:" || p[1] != "EVT" || p[2] != "ROOM" {
            continue;
        }

        match p[3] {
            "PRESENCE" if p.len() >= 6 => {
                let name = p[5];
                match p[4] {
                    "ENTER" | "ROSTER" if p.len() >= 9 => {
                        let skin = p[6];
                        let server_pos =
                            Vec2::new(p[7].parse().unwrap_or(0.0), p[8].parse().unwrap_or(0.0));

                        if Some(name) == local_name.0.as_deref() {
                            // On death: force the respawn coords regardless of what the server sent.
                            // On normal movement: use the server position as-is.
                            let final_pos = if pending_death.0 {
                                pending_death.0 = false;
                                DEATH_RESPAWN_POINT
                            } else {
                                server_pos
                            };

                            if let Ok((mut transform, mut anim, mut texture)) =
                                local_query.get_single_mut()
                            {
                                transform.translation.x = final_pos.x;
                                transform.translation.y = final_pos.y;
                                animate(
                                    &mut anim,
                                    &mut texture,
                                    Duration::from_millis(0),
                                    false,
                                    Vec2::ZERO,
                                );
                            }
                            continue;
                        }

                        if remotes.iter().any(|(_, r)| r.name == name) {
                            continue;
                        }
                        let entity =
                            spawn_player_visual(&mut commands, &asset_server, skin, name, server_pos);
                        commands.entity(entity).insert(RemotePlayer {
                            name: name.to_string(),
                            target: server_pos,
                        });
                    }
                    "LEAVE" => {
                        if Some(name) == local_name.0.as_deref() {
                            continue;
                        }
                        for (entity, r) in &remotes {
                            if r.name == name {
                                commands.entity(entity).despawn_recursive();
                            }
                        }
                    }
                    _ => {}
                }
            }
            "POS" if p.len() >= 7 => {
                let name = p[4];
                if Some(name) == local_name.0.as_deref() {
                    continue;
                }
                let target = Vec2::new(p[5].parse().unwrap_or(0.0), p[6].parse().unwrap_or(0.0));
                for (_, mut r) in &mut remotes {
                    if r.name == name {
                        r.target = target;
                    }
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn update_remote_players(
    time: Res<Time>,
    mut query: Query<(
        &mut Transform,
        &RemotePlayer,
        &mut PlayerAnimation,
        &mut Handle<Image>,
    )>,
) {
    for (mut transform, remote, mut anim, mut texture) in &mut query {
        let current = transform.translation.truncate();
        let to_target = remote.target - current;
        let distance = to_target.length();

        if distance > 200.0 {
            // Teleport instantly if the distance is too large (e.g., map transition)
            transform.translation.x = remote.target.x;
            transform.translation.y = remote.target.y;
            animate(&mut anim, &mut texture, time.delta(), false, Vec2::ZERO);
        } else if distance > 1.0 {
            let step = (PLAYER_SPEED * time.delta_seconds()).min(distance);
            let dir = to_target / distance;
            transform.translation.x += dir.x * step;
            transform.translation.y += dir.y * step;
            animate(&mut anim, &mut texture, time.delta(), true, dir);
        } else {
            animate(&mut anim, &mut texture, time.delta(), false, Vec2::ZERO);
        }
    }
}
