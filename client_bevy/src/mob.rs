use std::time::Duration;

use bevy::prelude::*;

use crate::map::YSort;
use crate::net::ServerMessageEvent;
use crate::player::PlayerName;
use crate::player::LocalPlayer;
use crate::AppState;

pub struct MobPlugin;

impl Plugin for MobPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InteractableTarget>()
            .init_resource::<GroundItemTarget>()
            .add_systems(Startup, setup_interaction_ui)
            .add_systems(OnEnter(AppState::MainMenu), clear_mobs_on_logout)
            .add_systems(
                Update,
                (
                    handle_mob_events,
                    handle_board_spawn_events,
                    animate_mobs,
                    cleanup_mobs_on_room_change,
                    handle_ground_item_events,
                    cleanup_items_on_room_change,
                    update_ground_item_target_impl,
                    interaction_system,
                    handle_interaction_input,
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[derive(Resource, Default)]
pub struct InteractableTarget(pub Option<String>);

#[derive(Component)]
pub struct InteractionPrompt;

const MOB_FOLDER: &str = "mob";
const INTERACTION_PROMPT_PNJ_SPRITE: &str = "Interaction/interact_pnj.png";
const INTERACTION_PROMPT_MOB_SPRITE: &str = "Interaction/interact_mob.png";
const MOB_RENDER_SIZE: f32 = 230.0;
const IDLE_FRAME_TIME: f32 = 0.18;

// Spritesheet layout: 2 columns × 3 rows = 6 frames
const SHEET_COLS: usize = 2;
const SHEET_ROWS: usize = 3;
const IDLE_FRAME_COUNT: usize = SHEET_COLS * SHEET_ROWS;

#[derive(Component)]
pub struct Mob {
    pub npc_id: String,
    pub is_enemy: bool,
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
            let npc_name = parts.get(11).map(|s| s.replace("_", " ")).unwrap_or_else(|| npc_id.to_string());
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
                    sheet_size: Vec2::ZERO, // will be filled once image loads
                },
                YSort,
            ));
            println!("[MOB] Spawned '{}' (name '{}', sprite '{}') at ({}, {}) scale: {}", npc_id, npc_name, sprite_clean, x, y, scale);
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
                // Image statique / pleine (ex: IdleSqueleton.png 500x500)
                anim.frames.push(Rect::from_corners(Vec2::ZERO, Vec2::new(img_w, img_h)));
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
fn clear_mobs_on_logout(
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
fn cleanup_mobs_on_room_change(
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

fn setup_interaction_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let pnj_texture = asset_server.load(INTERACTION_PROMPT_PNJ_SPRITE);
    let mob_texture = asset_server.load(INTERACTION_PROMPT_MOB_SPRITE);
    commands.insert_resource(InteractionPromptSprites {
        pnj_texture: pnj_texture.clone(),
        mob_texture,
    });

    commands.spawn((
        SpriteBundle {
            texture: pnj_texture,
            sprite: Sprite {
                custom_size: Some(Vec2::new(126.0, 126.0)),
                ..default()
            },
            transform: Transform::from_xyz(0.0, 0.0, 900.0),
            visibility: Visibility::Hidden,
            ..default()
        },
        InteractionPrompt,
    ));
}

#[derive(Resource, Clone)]
struct InteractionPromptSprites {
    pnj_texture: Handle<Image>,
    mob_texture: Handle<Image>,
}

fn interaction_system(
    player_q: Query<&Transform, With<LocalPlayer>>,
    mob_q: Query<(&Transform, &Mob), Without<InteractionPrompt>>,
    board_q: Query<(&Transform, &BoardNode), Without<InteractionPrompt>>,
    items_q: Query<(&Transform, &GroundItem), Without<InteractionPrompt>>,
    mut prompt_q: Query<(&mut Visibility, &mut Transform, &mut Handle<Image>), (With<InteractionPrompt>, Without<LocalPlayer>, Without<Mob>, Without<BoardNode>, Without<GroundItem>)>,
    mut interactable: ResMut<InteractableTarget>,
    ground_target: Res<GroundItemTarget>,
    combat: Res<crate::combat::CombatResource>,
    dialogue: Res<crate::interaction::DialogueState>,
    prompt_sprites: Res<InteractionPromptSprites>,
) {
    if combat.in_combat || dialogue.is_active() {
        if let Ok((mut vis, _, _)) = prompt_q.get_single_mut() {
            *vis = Visibility::Hidden;
        }
        interactable.0 = None;
        return;
    }
    let Ok(player_transform) = player_q.get_single() else { return; };
    let mut closest_dist = f32::MAX;
    let mut closest_npc = None;
    let mut closest_pos = Vec3::ZERO;
    let mut closest_is_enemy = false;

    for (mob_transform, mob) in &mob_q {
        let dist = player_transform.translation.distance(mob_transform.translation);
        if dist < 150.0 && dist < closest_dist {
            closest_dist = dist;
            closest_npc = Some(mob.npc_id.clone());
            closest_pos = mob_transform.translation;
            closest_is_enemy = mob.is_enemy;
        }
    }

    // Also check board nodes (notice boards — no enemy, use PNJ prompt icon).
    for (board_transform, board) in &board_q {
        let dist = player_transform.translation.distance(board_transform.translation);
        if dist < 150.0 && dist < closest_dist {
            closest_dist = dist;
            closest_npc = Some(board.npc_id.clone());
            closest_pos = board_transform.translation;
            closest_is_enemy = false;
        }
    }

    // Also show the prompt icon for nearby ground items (use PNJ bubble).
    if closest_npc.is_none() {
        if ground_target.0.is_some() {
            // Find the item entity position to float the bubble over it.
            for (tf, gi) in &items_q {
                if ground_target.0.as_deref() == Some(&gi.item_id) {
                    // Use only x/y from the item; override z so the bubble renders on top.
                    closest_pos = Vec3::new(tf.translation.x, tf.translation.y, 0.0);
                    closest_is_enemy = false;
                    break;
                }
            }
        }
    }

    let show_ground_prompt = closest_npc.is_none() && ground_target.0.is_some();
    interactable.0 = closest_npc;

    if let Ok((mut vis, mut prompt_transform, mut prompt_texture)) = prompt_q.get_single_mut() {
        if interactable.0.is_some() || show_ground_prompt {
            *vis = Visibility::Visible;
            *prompt_texture = if closest_is_enemy {
                prompt_sprites.mob_texture.clone()
            } else {
                prompt_sprites.pnj_texture.clone()
            };
            // Float above the mob / board / item
            prompt_transform.translation = closest_pos + Vec3::new(0.0, 60.0, 900.0);
        } else {
            *vis = Visibility::Hidden;
        }
    }
}

fn handle_interaction_input(
    input: Res<ButtonInput<KeyCode>>,
    interactable: Res<InteractableTarget>,
    ground_target: Res<GroundItemTarget>,
    combat: Res<crate::combat::CombatResource>,
    dialogue: Res<crate::interaction::DialogueState>,
    sender: Res<crate::net::NetworkSender>,
) {
    if combat.in_combat || combat.phase != crate::combat::CombatTransitionPhase::None || dialogue.is_active() {
        return;
    }
    if input.just_pressed(KeyCode::KeyE) {
        if let Some(ref item_id) = ground_target.0 {
            // Ground item takes priority if it is closer
            let _ = sender.0.send(format!("TAKE {}\n", item_id));
        } else if let Some(ref npc_id) = interactable.0 {
            let _ = sender.0.send(format!("INTERACT {}\n", npc_id));
        }
    }
}

// ── Notice Board ─────────────────────────────────────────────────────────────

/// Marks an invisible board interaction point (no sprite, just a position).
#[derive(Component)]
pub struct BoardNode {
    pub npc_id: String,
}

/// Spawn/despawn board nodes from `S: EVT ROOM BOARD_SPAWN <id> <x> <y>`.
fn handle_board_spawn_events(
    mut commands: Commands,
    mut events: EventReader<crate::net::ServerMessageEvent>,
    boards: Query<(Entity, &BoardNode)>,
) {
    for ev in events.read() {
        let parts: Vec<&str> = ev.0.split_whitespace().collect();

        // S: EVT ROOM BOARD_SPAWN <npc_id> <x> <y>
        if parts.len() >= 7
            && parts[0] == "S:" && parts[1] == "EVT"
            && parts[2] == "ROOM" && parts[3] == "BOARD_SPAWN"
        {
            let npc_id = parts[4];
            if boards.iter().any(|(_, b)| b.npc_id == npc_id) { continue; }
            let x: f32 = parts[5].parse().unwrap_or(0.0);
            let y: f32 = parts[6].parse().unwrap_or(0.0);
            // No sprite — just an invisible transform node for interaction detection.
            commands.spawn((
                TransformBundle::from_transform(Transform::from_xyz(x, y, 0.0)),
                VisibilityBundle::default(),
                BoardNode { npc_id: npc_id.to_string() },
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

// ── Ground Items ──────────────────────────────────────────────────────────────

/// A ground item rendered in the game world.
#[derive(Component)]
pub struct GroundItem {
    pub item_id: String,
}

/// The closest interactable ground item to the player, if any.
#[derive(Resource, Default)]
pub struct GroundItemTarget(pub Option<String>);

const GROUND_ITEM_SIZE: f32 = 48.0;
const GROUND_ITEM_Z: f32 = 400.0;
const ITEM_INTERACT_DIST: f32 = 140.0;

fn handle_ground_item_events(
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
            && parts[0] == "S:" && parts[1] == "EVT"
            && parts[2] == "ROOM" && parts[3] == "ITEM_SPAWN"
        {
            let item_id = parts[4];
            // Skip if already in scene
            if existing.iter().any(|(_, gi)| gi.item_id == item_id) { continue; }

            let sprite_path = parts[6];
            let x: f32 = parts[7].parse().unwrap_or(0.0);
            let y: f32 = parts[8].parse().unwrap_or(0.0);

            let texture: Handle<Image> = if sprite_path != "none" {
                asset_server.load(format!("{}.png", sprite_path))
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
                GroundItem { item_id: item_id.to_string() },
            ));
        }

        // S: EVT ROOM ITEM_DESPAWN <item_id>
        if parts.len() >= 5
            && parts[0] == "S:" && parts[1] == "EVT"
            && parts[2] == "ROOM" && parts[3] == "ITEM_DESPAWN"
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
fn update_ground_item_target_impl(
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
    let Ok(player_tf) = player_q.get_single() else { return };
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
fn cleanup_items_on_room_change(
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
