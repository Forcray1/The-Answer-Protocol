use bevy::prelude::*;
use crate::map::YSort;
use crate::net::ServerMessageEvent;
use crate::player::PlayerName;
use crate::mob::components::{IDLE_FRAME_TIME, InteractableTarget, MOB_FOLDER, MOB_RENDER_SIZE, Mob, MobAnimation, SHEET_COLS, SHEET_ROWS};

pub(crate) fn handle_mob_events(
    mut commands: Commands,
    mut events: EventReader<ServerMessageEvent>,
    asset_server: Res<AssetServer>,
    mobs: Query<(Entity, &Mob)>,
) {
    for ev in events.read() {
        let parts: Vec<&str> = ev.0.split_whitespace().collect();

        // S: EVT ROOM MOB_SPAWN <npc_id> <sprite> <x> <y> <hp> <max_hp> <scale> <name> [role]
        if parts.len() >= 9
            && parts[0] == "S:"
            && parts[1] == "EVT"
            && parts[2] == "ROOM"
            && parts[3] == "MOB_SPAWN"
        {
            let npc_id = parts[4];
            let sprite_name = parts[5];
            let x: f32 = parts[6].parse().unwrap_or(0.0);
            let y: f32 = parts[7].parse().unwrap_or(0.0);
            let scale: f32 = parts.get(10).and_then(|s| s.parse().ok()).unwrap_or(1.0);
            let npc_name = parts
                .get(11)
                .map(|s| s.replace("_", " "))
                .unwrap_or_else(|| npc_id.to_string());
            let is_enemy = parts.get(12).map(|s| *s == "enemy").unwrap_or(false);

            // Don't duplicate if already spawned
            if mobs.iter().any(|(_, m)| m.npc_id == npc_id) {
                continue;
            }

            let sprite_clean = sprite_name.strip_suffix(".png").unwrap_or(sprite_name);
            let texture: Handle<Image> =
                asset_server.load(format!("{}/{}.png", MOB_FOLDER, sprite_clean));

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
                    is_enemy,
                },
                PlayerName(npc_name.clone()),
                MobAnimation {
                    frames,
                    frame_index: 0,
                    timer: Timer::from_seconds(IDLE_FRAME_TIME, TimerMode::Repeating),
                },
                YSort,
            ));
            println!(
                "[MOB] Spawned '{}' (name '{}', sprite '{}') at ({}, {}) scale: {}",
                npc_id, npc_name, sprite_clean, x, y, scale
            );
        }

        // S: EVT ROOM MOB_DESPAWN <npc_id>
        if parts.len() >= 5
            && parts[0] == "S:"
            && parts[1] == "EVT"
            && parts[2] == "ROOM"
            && parts[3] == "MOB_DESPAWN"
        {
            let npc_id = parts[4];
            for (entity, mob) in &mobs {
                if mob.npc_id == npc_id {
                    commands.entity(entity).despawn_recursive();
                    println!("[MOB] Despawned '{}'", npc_id);
                }
            }
        }
    }
}

pub(crate) fn animate_mobs(
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

            // If ratio matches a 2x3 spritesheet (e.g. 120x180, 100x150)
            let is_2x3_sheet = (img_w * 3.0 - img_h * 2.0).abs() < 5.0 && img_w != img_h;
            if is_2x3_sheet {
                let frame_w = img_w / SHEET_COLS as f32;
                let frame_h = img_h / SHEET_ROWS as f32;

                for row in 0..SHEET_ROWS {
                    for col in 0..SHEET_COLS {
                        let min = Vec2::new(col as f32 * frame_w, row as f32 * frame_h);
                        let max = Vec2::new(min.x + frame_w, min.y + frame_h);
                        anim.frames.push(Rect::from_corners(min, max));
                    }
                }
            } else {
                anim.frames
                    .push(Rect::from_corners(Vec2::ZERO, Vec2::new(img_w, img_h)));
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

/// Back to the login screen: remove every mob of the previous session.
pub(crate) fn clear_mobs_on_logout(
    mut commands: Commands,
    mobs: Query<Entity, With<Mob>>,
    mut target: ResMut<InteractableTarget>,
) {
    for entity in &mobs {
        commands.entity(entity).despawn_recursive();
    }
    target.0 = None;
}

/// When the client enters a new room, despawn all existing mobs.
/// The server will send fresh MOB_SPAWN events for the new room.
pub(crate) fn cleanup_mobs_on_room_change(
    mut commands: Commands,
    mut events: EventReader<ServerMessageEvent>,
    mobs: Query<Entity, With<Mob>>,
) {
    for ev in events.read() {
        if ev.0.starts_with("S: OK room=loc.") {
            for entity in &mobs {
                commands.entity(entity).despawn_recursive();
            }
        }
    }
}
