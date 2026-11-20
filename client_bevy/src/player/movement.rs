use std::time::Duration;
use bevy::prelude::*;
use crate::AppState;
use crate::loading::{RoomTransition, RoomTransitionRequest};
use crate::map::YSort;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::animation::animate;
use crate::player::arrival::arrival_point;
use crate::player::components::{LocalPlayer, PlayerAnimation, PosSendTimer};
use crate::player::config::{FOOT_HALF_H, FOOT_HALF_W, FOOT_OFFSET_Y, MAP_HALF_H, MAP_HALF_W, PLAYER_SPEED};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dir {
    North,
    South,
    East,
    West,
}

impl Dir {
    pub(crate) fn vec(self) -> Vec2 {
        match self {
            Dir::North => Vec2::new(0.0, 1.0),
            Dir::South => Vec2::new(0.0, -1.0),
            Dir::East => Vec2::new(1.0, 0.0),
            Dir::West => Vec2::new(-1.0, 0.0),
        }
    }

    pub(crate) fn is_just_pressed(self, keys: &ButtonInput<KeyCode>) -> bool {
        let (primary, secondary) = match self {
            Dir::North => (KeyCode::KeyW, KeyCode::KeyZ),
            Dir::South => (KeyCode::KeyS, KeyCode::KeyS),
            Dir::West => (KeyCode::KeyA, KeyCode::KeyQ),
            Dir::East => (KeyCode::KeyD, KeyCode::KeyD),
        };
        keys.just_pressed(primary) || keys.just_pressed(secondary)
    }

    pub(crate) fn is_pressed(self, keys: &ButtonInput<KeyCode>) -> bool {
        let (primary, secondary) = match self {
            Dir::North => (KeyCode::KeyW, KeyCode::KeyZ),
            Dir::South => (KeyCode::KeyS, KeyCode::KeyS),
            Dir::West => (KeyCode::KeyA, KeyCode::KeyQ),
            Dir::East => (KeyCode::KeyD, KeyCode::KeyD),
        };
        keys.pressed(primary) || keys.pressed(secondary)
    }
}

pub(crate) const MOVE_KEYS: [Dir; 4] = [Dir::North, Dir::South, Dir::West, Dir::East];

#[derive(Resource, Default)]
pub(crate) struct HeldDirections(pub(crate) Vec<Dir>);

pub(crate) fn update_local_player(
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<crate::ui::ChatConsole>,
    combat: Res<crate::combat::CombatResource>,
    dialogue: Res<crate::interaction::DialogueState>,
    time: Res<Time>,
    game_state: Res<crate::game::GameState>,
    transition: Res<RoomTransition>,
    collision: Res<crate::collision::CollisionMask>,
    mut held: ResMut<HeldDirections>,
    mut last_tp: Local<f32>,
    mut transition_events: EventWriter<RoomTransitionRequest>,
    mut query: Query<(&mut Transform, &mut PlayerAnimation, &mut Handle<Image>), With<LocalPlayer>>,
) {
    let Ok((mut transform, mut anim, mut texture)) = query.get_single_mut() else {
        return;
    };

    // Freeze player during transitions or combat.
    if transition.is_active()
        || combat.in_combat
        || combat.phase != crate::combat::CombatTransitionPhase::None
        || dialogue.is_active()
    {
        held.0.clear();
        animate(&mut anim, &mut texture, time.delta(), false, Vec2::ZERO);
        return;
    }

    if console.open || console.terminal_open {
        held.0.clear();
    } else {
        for d in MOVE_KEYS {
            if d.is_just_pressed(&keys) && !held.0.contains(&d) {
                held.0.push(d);
            }
        }
        held.0.retain(|d| d.is_pressed(&keys));
    }

    if let Some(&d) = held.0.first() {
        let delta = d.vec() * PLAYER_SPEED * time.delta_seconds();
        let cur_x = transform.translation.x;
        let cur_y = transform.translation.y;

        let half = Vec2::new(FOOT_HALF_W, FOOT_HALF_H);
        let foot = |x: f32, y: f32| Vec2::new(x, y + FOOT_OFFSET_Y);

        let try_x = cur_x + delta.x;
        let res_x = if collision.blocks_box(foot(try_x, cur_y), half) {
            cur_x
        } else {
            try_x
        };
        let try_y = cur_y + delta.y;
        let res_y = if collision.blocks_box(foot(res_x, try_y), half) {
            cur_y
        } else {
            try_y
        };

        let mut new_x = res_x;
        let mut new_y = res_y;

        let can_tp = time.elapsed_seconds() - *last_tp > 1.0;

        if new_y > MAP_HALF_H {
            if game_state.exits.contains(&"north".to_string()) && can_tp {
                let a = arrival_point(&game_state.current_room, "north");
                transition_events.send(RoomTransitionRequest {
                    direction: "north".to_string(),
                    arrival_point: a,
                });
                *last_tp = time.elapsed_seconds();
                new_y = MAP_HALF_H;
            } else {
                new_y = MAP_HALF_H;
            }
        } else if new_y < -MAP_HALF_H {
            if game_state.exits.contains(&"south".to_string()) && can_tp {
                let a = arrival_point(&game_state.current_room, "south");
                transition_events.send(RoomTransitionRequest {
                    direction: "south".to_string(),
                    arrival_point: a,
                });
                *last_tp = time.elapsed_seconds();
                new_y = -MAP_HALF_H;
            } else {
                new_y = -MAP_HALF_H;
            }
        }

        if new_x > MAP_HALF_W {
            if game_state.exits.contains(&"east".to_string()) && can_tp {
                let a = arrival_point(&game_state.current_room, "east");
                transition_events.send(RoomTransitionRequest {
                    direction: "east".to_string(),
                    arrival_point: a,
                });
                *last_tp = time.elapsed_seconds();
                new_x = MAP_HALF_W;
            } else {
                new_x = MAP_HALF_W;
            }
        } else if new_x < -MAP_HALF_W {
            if game_state.exits.contains(&"west".to_string()) && can_tp {
                let a = arrival_point(&game_state.current_room, "west");
                transition_events.send(RoomTransitionRequest {
                    direction: "west".to_string(),
                    arrival_point: a,
                });
                *last_tp = time.elapsed_seconds();
                new_x = -MAP_HALF_W;
            } else {
                new_x = -MAP_HALF_W;
            }
        }

        transform.translation.x = new_x;
        transform.translation.y = new_y;
        animate(&mut anim, &mut texture, time.delta(), true, d.vec());
    } else {
        animate(&mut anim, &mut texture, time.delta(), false, Vec2::ZERO);
    }
}

pub(crate) fn send_local_position(
    time: Res<Time>,
    mut timer: ResMut<PosSendTimer>,
    sender: Res<NetworkSender>,
    combat: Res<crate::combat::CombatResource>,
    query: Query<&Transform, With<LocalPlayer>>,
    mut last_sent: Local<Option<Vec2>>,
) {
    if combat.in_combat || combat.phase != crate::combat::CombatTransitionPhase::None {
        return;
    }
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }
    let Ok(transform) = query.get_single() else {
        return;
    };
    let pos = transform.translation.truncate();
    if last_sent.map_or(false, |p| p.distance(pos) < 1.0) {
        return; // Position unchanged: nothing to send.
    }
    *last_sent = Some(pos);
    let _ = sender.0.send(format!(
        "POS {} {}\n",
        pos.x.round() as i64,
        pos.y.round() as i64
    ));
}
