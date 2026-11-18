use bevy::prelude::*;

use crate::AppState;
use crate::net::{NetworkSender, ServerMessageEvent};

pub const MINIMAP_ASSET: &str = "maps/map.png";

const MINIMAP_KEY: KeyCode = KeyCode::KeyM;

const MAP_MARGIN_PCT: f32 = 5.0;

const BACKDROP_COLOR: Color = Color::rgba(0.0, 0.0, 0.0, 0.75);

const OVERLAY_Z: i32 = 1000;

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MinimapPlayerCounts>()
            .insert_resource(MinimapWhoRefresh(Timer::from_seconds(
                1.0,
                TimerMode::Repeating,
            )))
            .add_systems(Startup, spawn_minimap)
            .add_systems(
                Update,
                (
                    toggle_minimap,
                    poll_who_while_minimap_open,
                    update_counts_from_who,
                    refresh_minimap_count_text,
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

// Marker on the overlay root. We toggle its visibility, which also
// hides/shows its children automatically.
#[derive(Component)]
struct MinimapOverlay;

#[derive(Component)]
struct MinimapPlayerCountText;

#[derive(Resource, Default)]
struct MinimapPlayerCounts {
    room: usize,
    server: usize,
}

#[derive(Resource)]
struct MinimapWhoRefresh(Timer);

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
            parent.spawn((
                TextBundle::from_section(
                    "Server: 0\nRoom: 0",
                    TextStyle {
                        font_size: 22.0,
                        color: Color::rgba(0.95, 0.95, 0.95, 1.0),
                        ..default()
                    },
                )
                .with_style(Style {
                    position_type: PositionType::Absolute,
                    top: Val::Px(16.0),
                    left: Val::Px(16.0),
                    ..default()
                }),
                MinimapPlayerCountText,
            ));
        });
}

// Opens/closes the map on M (ignored while chat is open, otherwise typing
// "m" in a message would open the map).
fn toggle_minimap(
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<crate::ui::ChatConsole>,
    mut query: Query<&mut Visibility, With<MinimapOverlay>>,
    sender: Option<Res<NetworkSender>>,
    combat: Option<Res<crate::combat::CombatResource>>,
) {
    if let Some(ref combat) = combat {
        if combat.in_combat {
            for mut visibility in &mut query {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
            }
            return;
        }
        if combat.phase != crate::combat::CombatTransitionPhase::None {
            return;
        }
    }

    if console.open || console.terminal_open || !keys.just_pressed(MINIMAP_KEY) {
        return;
    }

    for mut visibility in &mut query {
        *visibility = match *visibility {
            Visibility::Hidden => Visibility::Visible,
            _ => Visibility::Hidden,
        };
        if *visibility == Visibility::Visible {
            if let Some(sender) = &sender {
                let _ = sender.0.send("WHO\n".to_string());
            }
        }
    }
}

fn poll_who_while_minimap_open(
    time: Res<Time>,
    mut refresh: ResMut<MinimapWhoRefresh>,
    sender: Option<Res<NetworkSender>>,
    query: Query<&Visibility, With<MinimapOverlay>>,
) {
    let Ok(visibility) = query.get_single() else {
        return;
    };
    if *visibility != Visibility::Visible {
        refresh.0.reset();
        return;
    }

    refresh.0.tick(time.delta());
    if refresh.0.just_finished() {
        if let Some(sender) = &sender {
            let _ = sender.0.send("WHO\n".to_string());
        }
    }
}

fn update_counts_from_who(
    mut events: EventReader<ServerMessageEvent>,
    mut counts: ResMut<MinimapPlayerCounts>,
) {
    for ev in events.read() {
        if let Some((room, server)) = parse_who_counts(&ev.0) {
            counts.room = room;
            counts.server = server;
        }
    }
}

fn refresh_minimap_count_text(
    counts: Res<MinimapPlayerCounts>,
    mut text_query: Query<&mut Text, With<MinimapPlayerCountText>>,
) {
    if !counts.is_changed() {
        return;
    }
    let Ok(mut text) = text_query.get_single_mut() else {
        return;
    };
    text.sections[0].value = format!("Server: {}\nRoom: {}", counts.server, counts.room);
}

fn parse_who_counts(message: &str) -> Option<(usize, usize)> {
    let payload = message.strip_prefix("S: OK ")?;
    if !payload.contains("\"server\":") || !payload.contains("\"room\":[") {
        return None;
    }

    let room_start = payload.find("\"room\":[")? + "\"room\":[".len();
    let room_end = payload[room_start..].find(']')? + room_start;
    let room_slice = payload[room_start..room_end].trim();
    let room_count = if room_slice.is_empty() {
        0
    } else {
        room_slice.split(',').count()
    };

    let server_tail = payload.split("\"server\":").nth(1)?;
    let server_digits: String = server_tail
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let server_count = server_digits.parse::<usize>().ok()?;

    Some((room_count, server_count))
}
