use bevy::prelude::*;

use crate::AppState;


pub const MINIMAP_ASSET: &str = "maps/map.png";

const MINIMAP_KEY: KeyCode = KeyCode::KeyM;

const MAP_MARGIN_PCT: f32 = 5.0;

const BACKDROP_COLOR: Color = Color::rgba(0.0, 0.0, 0.0, 0.75);

const OVERLAY_Z: i32 = 1000;


pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_minimap)
            .add_systems(Update, toggle_minimap.run_if(in_state(AppState::InGame)));
    }
}

// Marker on the overlay root. We toggle its visibility, which also
// hides/shows its children automatically.
#[derive(Component)]
struct MinimapOverlay;

// Spawns the overlay once at startup, hidden by default.
fn spawn_minimap(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    position_type: PositionType::Absolute,
                    top: Val::Px(0.0),
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    ..default()
                },
                background_color: BACKDROP_COLOR.into(),
                visibility: Visibility::Hidden,
                z_index: ZIndex::Global(OVERLAY_Z),
                ..default()
            },
            MinimapOverlay,
        ))
        .with_children(|parent| {
            parent.spawn(ImageBundle {
                style: Style {
                    // Same margin on all 4 sides: centering is guaranteed by symmetry
                    position_type: PositionType::Absolute,
                    top: Val::Percent(MAP_MARGIN_PCT),
                    left: Val::Percent(MAP_MARGIN_PCT),
                    right: Val::Percent(MAP_MARGIN_PCT),
                    bottom: Val::Percent(MAP_MARGIN_PCT),
                    ..default()
                },
                image: UiImage::new(asset_server.load(MINIMAP_ASSET)),
                ..default()
            });
        });
}

// Opens/closes the map on M (ignored while chat is open, otherwise typing
// "m" in a message would open the map).
fn toggle_minimap(
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<crate::ui::ChatConsole>,
    mut query: Query<&mut Visibility, With<MinimapOverlay>>,
) {
    if console.open || console.terminal_open || !keys.just_pressed(MINIMAP_KEY) {
        return;
    }

    for mut visibility in &mut query {
        *visibility = match *visibility {
            Visibility::Hidden => Visibility::Visible,
            _ => Visibility::Hidden,
        };
    }
}