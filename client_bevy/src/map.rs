use bevy::prelude::*;
use bevy::render::camera::ScalingMode;

use crate::game::GameState;
use crate::AppState;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentZone>()
            .add_systems(Startup, setup_camera)
            .add_systems(Update, sync_map_to_room.run_if(in_state(AppState::InGame)))
            .add_systems(PostUpdate, y_sort_entities.run_if(in_state(AppState::InGame)));
    }
}

const ENTITY_Z_BASE: f32 = 500.0;
const ENTITY_Y_SORT_SLOPE: f32 = 0.1;
const OVERHEAD_Z_BASE: f32 = 900.0;

const MAP_WIDTH: f32 = 2560.0;
const MAP_HEIGHT: f32 = 1440.0;

const ASSET_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../sprites");
const OVERHEAD_SUBDIR: &str = "overhead";

#[derive(Component)]
struct MapLayer;

#[derive(Resource, Default)]
struct CurrentZone(Option<String>);

#[derive(Component)]
pub struct YSort;

fn setup_camera(mut commands: Commands) {
    let mut camera = Camera2dBundle::default();
    camera.projection.scaling_mode = ScalingMode::AutoMin {
        min_width: MAP_WIDTH,
        min_height: MAP_HEIGHT,
    };
    camera.projection.far = 10_000.0;
    camera.projection.near = -10_000.0;
    camera.tonemapping = bevy::core_pipeline::tonemapping::Tonemapping::None;
    commands.spawn(camera);
    println!("[CLIENT] 2D camera initialized, ready for precomputed assets.");
}

fn sync_map_to_room(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    game_state: Res<GameState>,
    mut current: ResMut<CurrentZone>,
    layers: Query<Entity, With<MapLayer>>,
) {
    let room = &game_state.current_room;

    if room == "unknown" || current.0.as_deref() == Some(room.as_str()) {
        return;
    }

    for entity in &layers {
        commands.entity(entity).despawn();
    }
    let (ground, overhead) = spawn_zone(&mut commands, &asset_server, room);
    println!(
        "[MAP] Zone '{}' built : {} ground + {} above",
        room, ground, overhead
    );
    current.0 = Some(room.clone());
}

fn spawn_zone(commands: &mut Commands, asset_server: &AssetServer, room: &str) -> (usize, usize) {
    let ground = list_layer_files(&format!("{}/maps/{}", ASSET_ROOT, room));
    for (i, file) in ground.iter().enumerate() {
        spawn_layer(
            commands,
            asset_server,
            format!("maps/{}/{}", room, file),
            i as f32 + 1.0,
        );
    }

    let overhead = list_layer_files(&format!("{}/maps/{}/{}", ASSET_ROOT, room, OVERHEAD_SUBDIR));
    for (i, file) in overhead.iter().enumerate() {
        spawn_layer(
            commands,
            asset_server,
            format!("maps/{}/{}/{}", room, OVERHEAD_SUBDIR, file),
            OVERHEAD_Z_BASE + i as f32 * 0.1,
        );
    }

    (ground.len(), overhead.len())
}

fn spawn_layer(commands: &mut Commands, asset_server: &AssetServer, asset_path: String, z: f32) {
    commands.spawn((
        SpriteBundle {
            texture: asset_server.load(asset_path),
            transform: Transform::from_xyz(0.0, 0.0, z),
            ..default()
        },
        MapLayer,
    ));
}

fn list_layer_files(abs_dir: &str) -> Vec<String> {
    let mut files: Vec<String> = match std::fs::read_dir(abs_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.to_ascii_lowercase().ends_with(".png"))
            .collect(),
        Err(_) => Vec::new(),
    };
    files.sort_by_key(|name| trailing_number(name));
    files
}

fn trailing_number(name: &str) -> u32 {
    let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
    let reversed_digits: String = stem.chars().rev().take_while(|c| c.is_ascii_digit()).collect();
    reversed_digits.chars().rev().collect::<String>().parse().unwrap_or(0)
}

fn y_sort_entities(mut query: Query<&mut Transform, With<YSort>>) {
    for mut transform in &mut query {
        transform.translation.z = ENTITY_Z_BASE - transform.translation.y * ENTITY_Y_SORT_SLOPE;
    }
}
