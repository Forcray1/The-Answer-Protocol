use bevy::prelude::*;
use crate::ui::hud::components::{HpBarFill, HpText, HudUiRoot, XpBarFill, XpText};

pub(crate) fn setup_hud_ui(mut commands: Commands) {
    // HUD Root - Top Left
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    top: Val::Px(20.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                visibility: Visibility::Hidden,
                z_index: ZIndex::Global(5),
                ..default()
            },
            HudUiRoot,
        ))
        .with_children(|root| {
            root.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(250.0),
                    height: Val::Px(25.0),
                    padding: UiRect::all(Val::Px(2.0)), // Inset for the fill
                    ..default()
                },
                background_color: Color::rgba(0.1, 0.1, 0.1, 0.8).into(),
                ..default()
            })
            .with_children(|bg| {
                // Fill
                bg.spawn((
                    NodeBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        background_color: Color::rgba(0.8, 0.2, 0.2, 0.9).into(), // Red
                        ..default()
                    },
                    HpBarFill,
                ));
                // Text overlay
                bg.spawn((
                    TextBundle::from_section(
                        "100/100",
                        TextStyle {
                            font_size: 16.0,
                            color: Color::WHITE,
                            ..default()
                        },
                    )
                    .with_style(Style {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(50.0),
                        top: Val::Percent(50.0),
                        margin: UiRect {
                            left: Val::Px(-30.0),
                            top: Val::Px(-8.0),
                            ..default()
                        },
                        ..default()
                    }),
                    HpText,
                ));
            });

            root.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(250.0),
                    height: Val::Px(25.0),
                    padding: UiRect::all(Val::Px(2.0)), // Inset for the fill
                    ..default()
                },
                background_color: Color::rgba(0.1, 0.1, 0.1, 0.8).into(),
                ..default()
            })
            .with_children(|bg| {
                // Fill
                bg.spawn((
                    NodeBundle {
                        style: Style {
                            width: Val::Percent(0.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        background_color: Color::rgba(0.2, 0.6, 0.8, 0.9).into(), // Blue
                        ..default()
                    },
                    XpBarFill,
                ));
                // Text overlay
                bg.spawn((
                    TextBundle::from_section(
                        "Lvl 1  0/100",
                        TextStyle {
                            font_size: 16.0,
                            color: Color::WHITE,
                            ..default()
                        },
                    )
                    .with_style(Style {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(50.0),
                        top: Val::Percent(50.0),
                        margin: UiRect {
                            left: Val::Px(-40.0),
                            top: Val::Px(-8.0),
                            ..default()
                        },
                        ..default()
                    }),
                    XpText,
                ));
            });
        });
}
