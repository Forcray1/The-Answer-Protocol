use std::time::Duration;

use bevy::prelude::*;

use crate::map::YSort;
use crate::net::ServerMessageEvent;
use crate::AppState;

pub struct MobPlugin;

impl Plugin for MobPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (handle_mob_events, animate_mobs, cleanup_mobs_on_room_change)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

const MOB_FOLDER: &str = "mob";
const MOB_RENDER_SIZE: f32 = 230.0;
const IDLE_FRAME_TIME: f32 = 0.18;

// Spritesheet layout: 2 columns × 3 rows = 6 frames
const SHEET_COLS: usize = 2;
const SHEET_ROWS: usize = 3;
const IDLE_FRAME_COUNT: usize = SHEET_COLS * SHEET_ROWS;

#[derive(Component)]
pub struct Mob {
    pub npc_id: String,
}

#[derive(Component)]
struct MobAnimation {
    frames: Vec<Rect>,
    frame_index: usize,
    timer: Timer,
    sheet_size: Vec2,
}

fn handle_mob_events(
    mut commands: Commands,
    mut events: EventReader<ServerMessageEvent>,
    asset_server: Res<AssetServer>,
    mobs: Query<(Entity, &Mob)>,
) {
    for ev in events.read() {
        let parts: Vec<&str> = ev.0.split_whitespace().collect();

        // S: EVT ROOM <room> MOB_SPAWN <npc_id> <sprite> <x> <y> <hp> <max_hp>
        if parts.len() >= 10
            && parts[0] == "S:"
            && parts[1] == "EVT"
            && parts[2] == "ROOM"
            && parts[4] == "MOB_SPAWN"
        {
            let npc_id = parts[5];
            let sprite_name = parts[6];
            let x: f32 = parts[7].parse().unwrap_or(0.0);
            let y: f32 = parts[8].parse().unwrap_or(0.0);
            let scale: f32 = parts.get(11).and_then(|s| s.parse().ok()).unwrap_or(1.0);

            // Don't duplicate if already spawned
            if mobs.iter().any(|(_, m)| m.npc_id == npc_id) {
                continue;
            }

            let texture: Handle<Image> =
                asset_server.load(format!("{}/{}.png", MOB_FOLDER, sprite_name));

            // We build frame rects lazily — we don't know the actual pixel size
            // yet (the image hasn't loaded), so we store normalized UVs once we
            // know the size. For now we store placeholder rects and will compute
            // them in the animate system on first tick using the image dimensions.
            let frames = Vec::new(); // filled on first animation tick

            commands.spawn((
                SpriteBundle {
                    texture,
                    sprite: Sprite {
                        custom_size: Some(Vec2::splat(MOB_RENDER_SIZE * scale)),
                        ..default()
                    },
                    transform: Transform::from_xyz(x, y, 0.0),
                    ..default()
                },
                Mob {
                    npc_id: npc_id.to_string(),
                },
                MobAnimation {
                    frames,
                    frame_index: 0,
                    timer: Timer::from_seconds(IDLE_FRAME_TIME, TimerMode::Repeating),
                    sheet_size: Vec2::ZERO, // will be filled once image loads
                },
                YSort,
            ));
            println!("[MOB] Spawned '{}' (sprite '{}') at ({}, {}) scale: {}", npc_id, sprite_name, x, y, scale);
        }

        // S: EVT ROOM <room> MOB_DESPAWN <npc_id>
        if parts.len() >= 6
            && parts[0] == "S:"
            && parts[1] == "EVT"
            && parts[2] == "ROOM"
            && parts[4] == "MOB_DESPAWN"
        {
            let npc_id = parts[5];
            for (entity, mob) in &mobs {
                if mob.npc_id == npc_id {
                    commands.entity(entity).despawn_recursive();
                    println!("[MOB] Despawned '{}'", npc_id);
                }
            }
        }
    }
}

fn animate_mobs(
    time: Res<Time>,
    images: Res<Assets<Image>>,
    mut query: Query<(&mut MobAnimation, &mut Sprite, &Handle<Image>)>,
) {
    for (mut anim, mut sprite, texture_handle) in &mut query {
        // Lazy-init: compute frame rects once the image is loaded
        if anim.frames.is_empty() {
            let Some(image) = images.get(texture_handle) else {
                continue; // image not yet loaded
            };
            let img_w = image.size().x as f32;
            let img_h = image.size().y as f32;
            anim.sheet_size = Vec2::new(img_w, img_h);

            let frame_w = img_w / SHEET_COLS as f32;
            let frame_h = img_h / SHEET_ROWS as f32;

            for row in 0..SHEET_ROWS {
                for col in 0..SHEET_COLS {
                    let min = Vec2::new(col as f32 * frame_w, row as f32 * frame_h);
                    let max = Vec2::new(min.x + frame_w, min.y + frame_h);
                    anim.frames.push(Rect::from_corners(min, max));
                }
            }

            // Set initial frame rect
            if let Some(rect) = anim.frames.first() {
                sprite.rect = Some(*rect);
            }
        }

        // Tick animation
        anim.timer.tick(time.delta());
        if anim.timer.just_finished() {
            anim.frame_index = (anim.frame_index + 1) % anim.frames.len();
            if let Some(rect) = anim.frames.get(anim.frame_index) {
                sprite.rect = Some(*rect);
            }
        }
    }
}

/// When the client enters a new room, despawn all existing mobs.
/// The server will send fresh MOB_SPAWN events for the new room.
fn cleanup_mobs_on_room_change(
    mut commands: Commands,
    mut events: EventReader<ServerMessageEvent>,
    mobs: Query<Entity, With<Mob>>,
) {
    for ev in events.read() {
        if ev.0.starts_with("S: OK room-loc.") {
            for entity in &mobs {
                commands.entity(entity).despawn_recursive();
            }
        }
    }
}
