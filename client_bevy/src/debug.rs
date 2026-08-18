use bevy::prelude::*;

use crate::game::GameState;
use crate::player::LocalPlayer;
use crate::AppState;

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugOverlay>()
            .add_systems(Startup, setup_debug_text)
            .add_systems(
                Update,
                (toggle_debug, update_debug_text).run_if(in_state(AppState::InGame)),
            );
    }
}

const DEBUG_TOGGLE_KEY: KeyCode = KeyCode::KeyO;

#[derive(Resource, Default)]
struct DebugOverlay {
    visible: bool,
}

#[derive(Component)]
struct DebugText;

fn setup_debug_text(mut commands: Commands) {
    commands.spawn((
        TextBundle::from_section(
            "",
            TextStyle {
                font_size: 22.0,
                color: Color::YELLOW,
                ..default()
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..default()
        }),
        DebugText,
    ));
}

fn toggle_debug(keys: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<DebugOverlay>) {
    if keys.just_pressed(DEBUG_TOGGLE_KEY) {
        overlay.visible = !overlay.visible;
    }
}

fn update_debug_text(
    overlay: Res<DebugOverlay>,
    game_state: Res<GameState>,
    player: Query<&Transform, With<LocalPlayer>>,
    camera: Query<(&Camera, &GlobalTransform)>,
    windows: Query<&Window>,
    mut text: Query<(&mut Text, &mut Visibility), With<DebugText>>,
) {
    let Ok((mut text, mut visibility)) = text.get_single_mut() else {
        return;
    };

    if !overlay.visible {
        *visibility = Visibility::Hidden;
        return;
    }
    *visibility = Visibility::Visible;

    let (px, py) = player
        .get_single()
        .map(|t| (t.translation.x, t.translation.y))
        .unwrap_or((0.0, 0.0));

    let mouse = windows
        .get_single()
        .ok()
        .and_then(|w| w.cursor_position())
        .and_then(|cursor| {
            let (cam, cam_tf) = camera.get_single().ok()?;
            cam.viewport_to_world_2d(cam_tf, cursor)
        });
    let mouse_str = match mouse {
        Some(m) => format!("Mouse: ({:.0}, {:.0})", m.x, m.y),
        None => "Mouse: (—)".to_string(),
    };

    text.sections[0].value = format!(
        "[F3] DEBUG\nRoom: {}\nPlayer: ({:.0}, {:.0})\n{}",
        game_state.current_room, px, py, mouse_str
    );
}
