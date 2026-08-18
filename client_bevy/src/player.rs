use std::time::Duration;

use bevy::prelude::*;

use crate::map::YSort;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::ui::ChatConsole;
use crate::AppState;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeldDirections>()
            .init_resource::<LocalPlayerName>()
            .insert_resource(PosSendTimer(Timer::from_seconds(POS_SEND_INTERVAL, TimerMode::Repeating)))
            .add_systems(Update, spawn_local_player_on_connect)
            .add_systems(
                Update,
                (
                    update_local_player,
                    send_local_position,
                    handle_presence_events,
                    update_remote_players,
                ).run_if(in_state(AppState::InGame)),
            );
    }
}

const SKIN_FOLDER: &str = "skin";
const DEFAULT_SKIN: &str = "default";
const DEFAULT_NAME: &str = "Player";
const CONNECT_OK_PREFIX: &str = "S: OK connected";

const DIR_FOLDERS: [&str; 4] = ["backward", "forward", "left", "right"];
const FRAME_COUNT: usize = 4;
const ANIM_FRAME_TIME: f32 = 0.12;

const PLAYER_SPEED: f32 = 400.0;
const PLAYER_RENDER_SIZE: f32 = 128.0;
const NAME_Y_OFFSET: f32 = 82.0;
const NAME_Z_OFFSET: f32 = 500.0;
const MAP_HALF_W: f32 = 1280.0;
const MAP_HALF_H: f32 = 720.0;

const FOOT_HALF_W: f32 = 28.0;
const FOOT_HALF_H: f32 = 12.0;
const FOOT_OFFSET_Y: f32 = -48.0;

const SPAWN_POINT: Vec2 = Vec2::new(-MAP_HALF_W + 80.0, 0.0);

const POS_SEND_INTERVAL: f32 = 0.1;

#[derive(Component)]
pub struct LocalPlayer;

#[derive(Component)]
struct RemotePlayer {
    name: String,
    target: Vec2,
}

#[derive(Component)]
struct PlayerAnimation {
    textures: [[Handle<Image>; FRAME_COUNT]; 4],
    facing: usize,
    frame: usize,
    timer: Timer,
}

#[derive(Resource, Default)]
struct LocalPlayerName(Option<String>);

#[derive(Resource)]
struct PosSendTimer(Timer);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dir {
    North,
    South,
    East,
    West,
}

impl Dir {
    fn vec(self) -> Vec2 {
        match self {
            Dir::North => Vec2::new(0.0, 1.0),
            Dir::South => Vec2::new(0.0, -1.0),
            Dir::East => Vec2::new(1.0, 0.0),
            Dir::West => Vec2::new(-1.0, 0.0),
        }
    }

    fn key(self) -> KeyCode {
        match self {
            Dir::North => KeyCode::KeyW,
            Dir::South => KeyCode::KeyS,
            Dir::West => KeyCode::KeyA,
            Dir::East => KeyCode::KeyD,
        }
    }
}

const MOVE_KEYS: [Dir; 4] = [Dir::North, Dir::South, Dir::West, Dir::East];

#[derive(Resource, Default)]
struct HeldDirections(Vec<Dir>);

fn spawn_local_player_on_connect(
    mut commands: Commands,
    mut events: EventReader<ServerMessageEvent>,
    asset_server: Res<AssetServer>,
    mut local_name: ResMut<LocalPlayerName>,
    existing: Query<(), With<LocalPlayer>>,
) {
    for ev in events.read() {
        let Some(rest) = ev.0.strip_prefix(CONNECT_OK_PREFIX) else {
            continue;
        };
        if !existing.is_empty() {
            continue;
        }

        let mut skin = DEFAULT_SKIN;
        let mut name = DEFAULT_NAME;
        let mut spawn = SPAWN_POINT;
        for token in rest.split_whitespace() {
            if let Some(v) = token.strip_prefix("skin=").filter(|v| !v.is_empty()) {
                skin = v;
            } else if let Some(v) = token.strip_prefix("name=").filter(|v| !v.is_empty()) {
                name = v;
            } else if let Some(v) = token.strip_prefix("pos=") {
                if let Some((xs, ys)) = v.split_once(',') {
                    if let (Ok(x), Ok(y)) = (xs.parse::<f32>(), ys.parse::<f32>()) {
                        spawn = Vec2::new(x, y);
                    }
                }
            }
        }

        local_name.0 = Some(name.to_string());
        let entity = spawn_player_visual(&mut commands, &asset_server, skin, name, spawn);
        commands.entity(entity).insert(LocalPlayer);
        println!("[PLAYER] Avatar local '{}' (skin '{}').", name, skin);
    }
}

fn spawn_player_visual(
    commands: &mut Commands,
    asset_server: &AssetServer,
    skin: &str,
    name: &str,
    pos: Vec2,
) -> Entity {
    let textures = load_skin_textures(asset_server, skin);

    commands
        .spawn((
            SpriteBundle {
                texture: textures[0][0].clone(),
                sprite: Sprite {
                    custom_size: Some(Vec2::splat(PLAYER_RENDER_SIZE)),
                    ..default()
                },
                transform: Transform::from_xyz(pos.x, pos.y, 0.0),
                ..default()
            },
            PlayerAnimation {
                textures,
                facing: 0,
                frame: 0,
                timer: Timer::from_seconds(ANIM_FRAME_TIME, TimerMode::Repeating),
            },
            YSort,
        ))
        .with_children(|parent| {
            parent.spawn(Text2dBundle {
                text: Text::from_section(
                    name,
                    TextStyle { font_size: 40.0, color: Color::WHITE, ..default() },
                ),
                transform: Transform::from_xyz(0.0, NAME_Y_OFFSET, NAME_Z_OFFSET),
                ..default()
            });
        })
        .id()
}

fn load_skin_textures(asset_server: &AssetServer, skin: &str) -> [[Handle<Image>; FRAME_COUNT]; 4] {
    std::array::from_fn(|dir| {
        std::array::from_fn(|frame| {
            asset_server.load(format!(
                "{}/{}/{}/f{}.png",
                SKIN_FOLDER, skin, DIR_FOLDERS[dir], frame + 1
            ))
        })
    })
}

fn default_arrival(dir: &str) -> Vec2 {
    match dir {
        "north" => Vec2::new(0.0, -MAP_HALF_H + 5.0),
        "south" => Vec2::new(0.0, MAP_HALF_H - 5.0),
        "east" => Vec2::new(-MAP_HALF_W + 5.0, 0.0),
        "west" => Vec2::new(MAP_HALF_W - 5.0, 0.0),
        _ => Vec2::ZERO,
    }
}

const C_TO_W6: Vec2 = Vec2::new(-950.0, 225.0);

fn arrival_point(from_room: &str, dir: &str) -> Vec2 {
    match (from_room, dir) {
        ("Cave", "east") => C_TO_W6,
        _ => default_arrival(dir),
    }
}

fn update_local_player(
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<crate::ui::ChatConsole>,
    time: Res<Time>,
    game_state: Res<crate::game::GameState>,
    sender: Res<crate::net::NetworkSender>,
    collision: Res<crate::collision::CollisionMask>,
    mut held: ResMut<HeldDirections>,
    mut last_tp: Local<f32>,
    mut query: Query<(&mut Transform, &mut PlayerAnimation, &mut Handle<Image>), With<LocalPlayer>>,
) {
    let Ok((mut transform, mut anim, mut texture)) = query.get_single_mut() else {
        return;
    };

    if console.open {
        held.0.clear();
    } else {
        for d in MOVE_KEYS {
            if keys.just_pressed(d.key()) && !held.0.contains(&d) {
                held.0.push(d);
            }
        }
        held.0.retain(|d| keys.pressed(d.key()));
    }

    if let Some(&d) = held.0.first() {
        let delta = d.vec() * PLAYER_SPEED * time.delta_seconds();
        let cur_x = transform.translation.x;
        let cur_y = transform.translation.y;

        let half = Vec2::new(FOOT_HALF_W, FOOT_HALF_H);
        let foot = |x: f32, y: f32| Vec2::new(x, y + FOOT_OFFSET_Y);

        let try_x = cur_x + delta.x;
        let res_x = if collision.blocks_box(foot(try_x, cur_y), half) { cur_x } else { try_x };
        let try_y = cur_y + delta.y;
        let res_y = if collision.blocks_box(foot(res_x, try_y), half) { cur_y } else { try_y };

        let mut new_x = res_x;
        let mut new_y = res_y;

        let can_tp = time.elapsed_seconds() - *last_tp > 1.0;

        if new_y > MAP_HALF_H {
            if game_state.exits.contains(&"north".to_string()) && can_tp {
                let _ = sender.0.send("MOVE north\n".to_string());
                let a = arrival_point(&game_state.current_room, "north");
                new_x = a.x;
                new_y = a.y;
                *last_tp = time.elapsed_seconds();
            } else {
                new_y = MAP_HALF_H;
            }
        } else if new_y < -MAP_HALF_H {
            if game_state.exits.contains(&"south".to_string()) && can_tp {
                let _ = sender.0.send("MOVE south\n".to_string());
                let a = arrival_point(&game_state.current_room, "south");
                new_x = a.x;
                new_y = a.y;
                *last_tp = time.elapsed_seconds();
            } else {
                new_y = -MAP_HALF_H;
            }
        }

        if new_x > MAP_HALF_W {
            if game_state.exits.contains(&"east".to_string()) && can_tp {
                let _ = sender.0.send("MOVE east\n".to_string());
                let a = arrival_point(&game_state.current_room, "east");
                new_x = a.x;
                new_y = a.y;
                *last_tp = time.elapsed_seconds();
            } else {
                new_x = MAP_HALF_W;
            }
        } else if new_x < -MAP_HALF_W {
            if game_state.exits.contains(&"west".to_string()) && can_tp {
                let _ = sender.0.send("MOVE west\n".to_string());
                let a = arrival_point(&game_state.current_room, "west");
                new_x = a.x;
                new_y = a.y;
                *last_tp = time.elapsed_seconds();
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

fn send_local_position(
    time: Res<Time>,
    mut timer: ResMut<PosSendTimer>,
    sender: Res<NetworkSender>,
    query: Query<&Transform, With<LocalPlayer>>,
    mut last_sent: Local<Option<Vec2>>,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }
    let Ok(transform) = query.get_single() else {
        return;
    };
    let pos = transform.translation.truncate();
    if last_sent.map_or(false, |p| p.distance(pos) < 1.0) {
        return; // position inchangée : rien à envoyer
    }
    *last_sent = Some(pos);
    let _ = sender.0.send(format!("POS {} {}\n", pos.x.round() as i64, pos.y.round() as i64));
}

fn handle_presence_events(
    mut commands: Commands,
    mut events: EventReader<ServerMessageEvent>,
    asset_server: Res<AssetServer>,
    local_name: Res<LocalPlayerName>,
    mut remotes: Query<(Entity, &mut RemotePlayer)>,
) {
    for ev in events.read() {
        if ev.0.starts_with("S: OK room-loc.") {
            for (entity, _) in &remotes {
                commands.entity(entity).despawn_recursive();
            }
            continue;
        }

        let p: Vec<&str> = ev.0.split_whitespace().collect();
        if p.len() < 5 || p[0] != "S:" || p[1] != "EVT" || p[2] != "ROOM" {
            continue;
        }

        match p[4] {
            "PRESENCE" if p.len() >= 7 => {
                let name = p[6];
                if Some(name) == local_name.0.as_deref() {
                    continue; // jamais soi-même
                }
                match p[5] {
                    "ENTER" if p.len() >= 10 => {
                        let skin = p[7];
                        let pos = Vec2::new(p[8].parse().unwrap_or(0.0), p[9].parse().unwrap_or(0.0));
                        if remotes.iter().any(|(_, r)| r.name == name) {
                            continue; // déjà présent
                        }
                        let entity = spawn_player_visual(&mut commands, &asset_server, skin, name, pos);
                        commands.entity(entity).insert(RemotePlayer {
                            name: name.to_string(),
                            target: pos,
                        });
                        println!("[PLAYER] Joueur distant '{}' apparu (skin '{}').", name, skin);
                    }
                    "LEAVE" => {
                        for (entity, r) in &remotes {
                            if r.name == name {
                                commands.entity(entity).despawn_recursive();
                            }
                        }
                    }
                    _ => {}
                }
            }
            "POS" if p.len() >= 8 => {
                let name = p[5];
                if Some(name) == local_name.0.as_deref() {
                    continue;
                }
                let target = Vec2::new(p[6].parse().unwrap_or(0.0), p[7].parse().unwrap_or(0.0));
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

fn update_remote_players(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &RemotePlayer, &mut PlayerAnimation, &mut Handle<Image>)>,
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

fn animate(
    anim: &mut PlayerAnimation,
    texture: &mut Handle<Image>,
    dt: Duration,
    moving: bool,
    dir: Vec2,
) {
    if moving {
        anim.facing = facing_index(dir);
        anim.timer.tick(dt);
        if anim.timer.just_finished() {
            anim.frame = (anim.frame + 1) % FRAME_COUNT;
        }
    } else {
        anim.timer.reset();
        anim.frame = 0;
    }

    let wanted = anim.textures[anim.facing][anim.frame].clone();
    if *texture != wanted {
        *texture = wanted;
    }
}

fn facing_index(dir: Vec2) -> usize {
    if dir.x < 0.0 {
        2
    } else if dir.x > 0.0 {
        3
    } else if dir.y > 0.0 {
        1
    } else {
        0
    }
}
