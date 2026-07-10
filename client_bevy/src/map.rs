//   [ SOL 1..=N ]   <   [ ENTITÉS ~430..570 ]   <   [ AU-DESSUS 900+ ]
//        │                      │                          │
//   calques de sol       joueurs / mobs            canopées, toits...
//                        (triés entre eux           (un perso peut passer
//                         par leur hauteur)          DERRIÈRE)

use bevy::prelude::*;
use bevy::render::camera::ScalingMode;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_camera, setup_map))
            .add_systems(PostUpdate, y_sort_entities);
    }
}


const ENTITY_Z_BASE: f32 = 500.0;

const ENTITY_Y_SORT_SLOPE: f32 = 0.1;

const OVERHEAD_Z_BASE: f32 = 900.0;


const MAP_WIDTH: f32 = 2560.0;
const MAP_HEIGHT: f32 = 1440.0;


const SPAWN_DEMO_MARKER: bool = false;


struct MapZone {
	folder: &'static str,
    file_pattern: &'static str,
    layer_count: u32,
    overhead_from: u32,
}

///   -- SOL (sous les entités) --        -- AU-DESSUS des entités --
///   1 = sable + sentier                 6 = palmier + buissons
///   2 = petite structure (coin)         7 = pancarte / caisse (quête)
///   3 = tente                           8 = dune de sable
///   4 = tentes + structures             9 = tente avec drapeau
///   5 = point d'eau                    10 = rocher (coin)
const OASIS: MapZone = MapZone {
    folder: "maps/oasis",
    file_pattern: "Map_1-6-{n}.png",
    layer_count: 10,
    overhead_from: 6,
};

#[derive(Component)]
struct MapLayer;

/// À AJOUTER sur toute entité de jeu (joueur, mob) qu'on veut trier en
/// profondeur selon sa hauteur à l'écran. Il suffit de spawner l'entité avec
/// ce composant : `y_sort_entities` s'occupe du reste chaque frame.
#[derive(Component)]
pub struct YSort;

fn setup_camera(mut commands: Commands) {
    let mut camera = Camera2dBundle::default();
    camera.projection.scaling_mode = ScalingMode::AutoMin {
        min_width: MAP_WIDTH,
        min_height: MAP_HEIGHT,
    };
    commands.spawn(camera);
    println!("[CLIENT] 2D camera initialized, ready for precomputed assets.");
}

fn setup_map(mut commands: Commands, asset_server: Res<AssetServer>) {
    let zone = &OASIS;

    for layer in 1..=zone.layer_count {
        let file = zone.file_pattern.replace("{n}", &layer.to_string());
        let path = format!("{}/{}", zone.folder, file);

		let z = if layer >= zone.overhead_from {
            OVERHEAD_Z_BASE + layer as f32 * 0.1
        } else {
            layer as f32
        };

        commands.spawn((
            SpriteBundle {
                texture: asset_server.load(path),
                transform: Transform::from_xyz(0.0, 0.0, z),
                ..default()
            },
            MapLayer,
        ));
    }

    println!(
        "[MAP] Zone '{}' charged : {} supperposed layers ({} on ground, {} on top of entities).",
        zone.folder,
        zone.layer_count,
        zone.overhead_from - 1,
        zone.layer_count - zone.overhead_from + 1,
    );

    if SPAWN_DEMO_MARKER {
        spawn_demo_marker(&mut commands);
    }
}

fn y_sort_entities(mut query: Query<&mut Transform, With<YSort>>) {
    for mut transform in &mut query {
        transform.translation.z = ENTITY_Z_BASE - transform.translation.y * ENTITY_Y_SORT_SLOPE;
    }
}

fn spawn_demo_marker(commands: &mut Commands) {
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0.9, 0.1, 0.1, 0.95),
                custom_size: Some(Vec2::new(120.0, 120.0)),
                ..default()
            },
            transform: Transform::from_xyz(190.0, 320.0, ENTITY_Z_BASE),
            ..default()
        },
        YSort,
    ));
}
