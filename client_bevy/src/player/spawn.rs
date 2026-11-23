use bevy::prelude::*;
use crate::map::YSort;
use crate::net::ServerMessageEvent;
use crate::player::components::{LocalPlayer, LocalPlayerName, LocalPlayerSkin, PendingDeathRespawn, PlayerAnimation, PlayerName, RemotePlayer};
use crate::player::config::{ANIM_FRAME_TIME, CONNECT_OK_PREFIX, DEFAULT_NAME, DEFAULT_SKIN, DIR_FOLDERS, FRAME_COUNT, NAME_Y_OFFSET, NAME_Z_OFFSET, PLAYER_RENDER_SIZE, SKIN_FOLDER, SPAWN_POINT};
use crate::player::movement::HeldDirections;

pub(crate) fn spawn_local_player_on_connect(
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
                if let Some((xs, ys)) = v.split_once(',') && let (Ok(x), Ok(y)) = (xs.parse::<f32>(), ys.parse::<f32>()) {
                    spawn = Vec2::new(x, y);
                }
            }
        }

        // Keep the spawn explicit and exact: the server is authoritative for the oasis.
        if spawn.x == 0.0 && spawn.y == 0.0 && name != DEFAULT_NAME {
            spawn = SPAWN_POINT;
        }

        local_name.0 = Some(name.to_string());
        commands.insert_resource(LocalPlayerSkin(skin.to_string()));
        let entity = spawn_player_visual(&mut commands, &asset_server, skin, name, spawn);
        commands.entity(entity).insert(LocalPlayer);
    }
}

pub(crate) fn spawn_player_visual(
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
            PlayerName(name.to_string()),
        ))
        .with_children(|parent| {
            parent.spawn(Text2dBundle {
                text: Text::from_section(
                    name,
                    TextStyle {
                        font_size: 40.0,
                        color: Color::WHITE,
                        ..default()
                    },
                ),
                transform: Transform::from_xyz(0.0, NAME_Y_OFFSET, NAME_Z_OFFSET),
                ..default()
            });
        })
        .id()
}

pub(crate) fn load_skin_textures(asset_server: &AssetServer, skin: &str) -> [[Handle<Image>; FRAME_COUNT]; 4] {
    std::array::from_fn(|dir| {
        std::array::from_fn(|frame| {
            asset_server.load(format!(
                "{}/{}/{}/f{}.png",
                SKIN_FOLDER,
                skin,
                DIR_FOLDERS[dir],
                frame + 1
            ))
        })
    })
}

/// Back to the login screen (QUIT or lost connection): remove the local avatar and the
/// remote players of the previous session, otherwise the next login would reuse them
/// (`spawn_local_player_on_connect` does nothing while a local player already exists).
pub(crate) fn clear_players_on_logout(
    mut commands: Commands,
    locals: Query<Entity, With<LocalPlayer>>,
    remotes: Query<Entity, With<RemotePlayer>>,
    mut local_name: ResMut<LocalPlayerName>,
    mut held: ResMut<HeldDirections>,
    mut pending_death: ResMut<PendingDeathRespawn>,
) {
    for entity in locals.iter().chain(remotes.iter()) {
        commands.entity(entity).despawn_recursive();
    }
    local_name.0 = None;
    held.0.clear();
    pending_death.0 = false;
}
