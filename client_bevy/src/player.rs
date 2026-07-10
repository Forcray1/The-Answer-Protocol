use std::time::Duration;

use bevy::prelude::*;

use crate::map::YSort;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::ui::ChatConsole;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeldDirections>()
            .init_resource::<LocalPlayerName>()
            .insert_resource(PosSendTimer(Timer::from_seconds(POS_SEND_INTERVAL, TimerMode::Repeating)))
            .add_systems(
                Update,
                (
                    spawn_local_player_on_connect,
                    update_local_player,
                    send_local_position,
                    handle_presence_events,
                    update_remote_players,
                ),
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
        for token in rest.split_whitespace() {
            if let Some(v) = token.strip_prefix("skin=").filter(|v| !v.is_empty()) {
                skin = v;
            } else if let Some(v) = token.strip_prefix("name=").filter(|v| !v.is_empty()) {
                name = v;
            }
        }

        local_name.0 = Some(name.to_string());
        let entity = spawn_player_visual(&mut commands, &asset_server, skin, name, Vec2::ZERO);
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

fn update_local_player(
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<ChatConsole>,
    time: Res<Time>,
    mut held: ResMut<HeldDirections>,
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
        transform.translation.x = (transform.translation.x + delta.x).clamp(-MAP_HALF_W, MAP_HALF_W);
        transform.translation.y = (transform.translation.y + delta.y).clamp(-MAP_HALF_H, MAP_HALF_H);
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

        if distance > 1.0 {
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
