use bevy::prelude::*;

use crate::map::YSort;
use crate::ServerMessageEvent;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_local_player_on_connect);
    }
}

const SKIN_FOLDER: &str = "skin";

const DEFAULT_SKIN: &str = "default";

const CONNECT_OK_PREFIX: &str = "S: OK connected";

#[derive(Component)]
pub struct LocalPlayer;

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

        let skin = rest
            .split_whitespace()
            .find_map(|token| token.strip_prefix("skin="))
            .filter(|name| !name.is_empty())
            .unwrap_or(DEFAULT_SKIN);

        spawn_local_player(&mut commands, &asset_server, skin);
    }
}

fn spawn_local_player(commands: &mut Commands, asset_server: &AssetServer, skin: &str) {
    let path = format!("{}/{}.png", SKIN_FOLDER, skin);

    commands.spawn((
        SpriteBundle {
            texture: asset_server.load(path.clone()),
            // Au centre de la carte pour l'instant : le serveur ne gère pas encore
            // de coordonnées (déplacement à câbler plus tard). Le Z sera fixé par
            // `y_sort_entities` grâce à `YSort`.
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..default()
        },
        LocalPlayer,
        YSort,
    ));
}
