use bevy::prelude::*;

use crate::game::GameState;
use crate::net::NetworkSender;
use crate::player::LocalPlayer;
use crate::AppState;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RoomTransition>()
            .add_event::<RoomTransitionRequest>()
            .add_systems(Startup, spawn_overlay)
            .add_systems(
                Update,
                (start_transition, animate_transition)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

// ── Constants ──────────────────────────────────────────────────────────

const FADE_DURATION: f32 = 0.7;
const OVERLAY_Z: f32 = 9999.0;
const OVERLAY_SIZE: f32 = 8000.0; // large enough to cover any camera view

// ── Events ─────────────────────────────────────────────────────────────

/// Fired by the player module when the player reaches a room edge.
#[derive(Event)]
pub struct RoomTransitionRequest {
    pub direction: String,
    pub arrival_point: Vec2,
}

// ── Resources ──────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct RoomTransition {
    pub phase: TransitionPhase,
}

#[derive(Default, Clone, PartialEq)]
pub enum TransitionPhase {
    #[default]
    None,
    /// Fading to black. `progress` goes 0 → 1.
    FadingOut {
        timer: f32,
        direction: String,
        from_room: String,
        arrival: Vec2,
    },
    /// Screen is black, waiting for GameState.current_room to change.
    WaitingForRoom {
        from_room: String,
        arrival: Vec2,
    },
    /// Fading from black back to normal. `progress` goes 1 → 0.
    FadingIn {
        timer: f32,
    },
}

impl RoomTransition {
    pub fn is_active(&self) -> bool {
        self.phase != TransitionPhase::None
    }
}

// ── Components ─────────────────────────────────────────────────────────

#[derive(Component)]
struct TransitionOverlay;

// ── Systems ────────────────────────────────────────────────────────────

fn spawn_overlay(mut commands: Commands) {
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0.0, 0.0, 0.0, 0.0),
                custom_size: Some(Vec2::splat(OVERLAY_SIZE)),
                ..default()
            },
            transform: Transform::from_xyz(0.0, 0.0, OVERLAY_Z),
            ..default()
        },
        TransitionOverlay,
    ));
}

/// Listens for transition requests from the player module.
fn start_transition(
    mut events: EventReader<RoomTransitionRequest>,
    mut transition: ResMut<RoomTransition>,
    game_state: Res<GameState>,
) {
    // Only process if no transition is already active.
    if transition.is_active() {
        events.clear();
        return;
    }

    for req in events.read() {
        // Don't send MOVE yet — wait until the screen is fully black.
        transition.phase = TransitionPhase::FadingOut {
            timer: 0.0,
            direction: req.direction.clone(),
            from_room: game_state.current_room.clone(),
            arrival: req.arrival_point,
        };
        println!(
            "[LOADING] Transition started → direction '{}' (from '{}')",
            req.direction, game_state.current_room
        );
        break; // one at a time
    }
}

/// Drives the fade animation and teleports the player at the right moment.
/// Also checks GameState to detect when the room has changed (handles the
/// race where the server responds before the fade-out completes).
fn animate_transition(
    time: Res<Time>,
    game_state: Res<GameState>,
    sender: Res<NetworkSender>,
    mut transition: ResMut<RoomTransition>,
    mut overlay_q: Query<&mut Sprite, With<TransitionOverlay>>,
    mut player_q: Query<&mut Transform, With<LocalPlayer>>,
) {
    let Ok(mut sprite) = overlay_q.get_single_mut() else {
        return;
    };

    match &mut transition.phase {
        TransitionPhase::None => {
            // Ensure overlay is invisible.
            sprite.color.set_a(0.0);
        }
        TransitionPhase::FadingOut {
            timer,
            direction,
            from_room,
            arrival,
        } => {
            *timer += time.delta_seconds();
            let t = (*timer / FADE_DURATION).min(1.0);
            sprite.color.set_a(t);

            if t >= 1.0 {
                // Screen is fully black → teleport player & send MOVE now.
                let arrival_copy = *arrival;
                let from_copy = from_room.clone();
                if let Ok(mut player_tf) = player_q.get_single_mut() {
                    player_tf.translation.x = arrival_copy.x;
                    player_tf.translation.y = arrival_copy.y;
                }
                let _ = sender.0.send(format!("MOVE {}\n", direction));
                println!("[LOADING] Fade-out complete, MOVE sent.");

                transition.phase = TransitionPhase::WaitingForRoom {
                    from_room: from_copy,
                    arrival: arrival_copy,
                };
            }
        }
        TransitionPhase::WaitingForRoom { from_room, .. } => {
            // Keep screen black while waiting.
            sprite.color.set_a(1.0);

            // Check if the room has changed in GameState.
            if game_state.current_room != *from_room {
                transition.phase = TransitionPhase::FadingIn { timer: 0.0 };
                println!("[LOADING] Room '{}' loaded, starting fade-in.", game_state.current_room);
            }
        }
        TransitionPhase::FadingIn { timer } => {
            *timer += time.delta_seconds();
            let t = (*timer / FADE_DURATION).min(1.0);
            sprite.color.set_a(1.0 - t);

            if t >= 1.0 {
                transition.phase = TransitionPhase::None;
                println!("[LOADING] Transition complete.");
            }
        }
    }
}
