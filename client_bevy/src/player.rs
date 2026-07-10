use bevy::prelude::*;

use crate::map::YSort;
use crate::{ChatConsole, ServerMessageEvent};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeldDirections>()
            .add_systems(Update, (spawn_local_player_on_connect, update_local_player));
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


#[derive(Component)]
pub struct LocalPlayer;

#[derive(Component)]
struct PlayerAnimation {
    textures: [[Handle<Image>; FRAME_COUNT]; 4],
    facing: usize,
    frame: usize,
    timer: Timer,
}

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

        spawn_local_player(&mut commands, &asset_server, skin, name);
    }
}

fn spawn_local_player(commands: &mut Commands, asset_server: &AssetServer, skin: &str, name: &str) {
    let textures: [[Handle<Image>; FRAME_COUNT]; 4] = std::array::from_fn(|dir| {
        std::array::from_fn(|frame| {
            asset_server.load(format!(
                "{}/{}/{}/f{}.png",
                SKIN_FOLDER, skin, DIR_FOLDERS[dir], frame + 1
            ))
        })
    });

    commands
        .spawn((
            SpriteBundle {
                texture: textures[0][0].clone(),
                sprite: Sprite {
                    custom_size: Some(Vec2::splat(PLAYER_RENDER_SIZE)),
                    ..default()
                },

                transform: Transform::from_xyz(0.0, 0.0, 0.0),
                ..default()
            },
            LocalPlayer,
            YSort,
            PlayerAnimation {
                textures,
                facing: 0,
                frame: 0,
                timer: Timer::from_seconds(ANIM_FRAME_TIME, TimerMode::Repeating),
            },
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
        });

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

        anim.facing = facing_index(d.vec());

        anim.timer.tick(time.delta());
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
