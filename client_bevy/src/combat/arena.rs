use crate::AppState;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::{LocalPlayerName, LocalPlayerSkin};
use crate::ui::PlayerStats;
use bevy::prelude::*;
use crate::combat::components::{CombatBtnSlot, CombatButtonVisual, CombatCursorVisual, CombatEnemySpritePos, CombatEnemyVisual, CombatEntity, CombatMessageText, CombatPlayerSprite, EnemyHpBarFill, EnemyHpText, PlayerHpBarFill, PlayerHpText};
use crate::combat::state::{COMBAT_Z_BG, COMBAT_Z_ENTITIES, CombatResource};

/// full-screen background image.
fn spawn_arena_background(
    commands: &mut Commands,
    asset_server: &AssetServer,
    combat: &CombatResource,
) {
    let bg_texture: Handle<Image> =
        if (combat.bg_index as usize) <= combat.bg_textures.len() && combat.bg_index > 0 {
            combat.bg_textures[(combat.bg_index - 1) as usize].clone()
        } else {
            asset_server.load(format!(
                "maps/FightMap/FightBackground{}.webp",
                combat.bg_index
            ))
        };

    // Background image
    commands.spawn((
        SpriteBundle {
            texture: bg_texture,
            sprite: Sprite {
                custom_size: Some(Vec2::new(2560.0, 1440.0)),
                ..default()
            },
            transform: Transform::from_xyz(0.0, 0.0, COMBAT_Z_BG),
            ..default()
        },
        CombatEntity,
    ));
}

/// player on the left (sprite, name tag and hp bar).
fn spawn_arena_player(
    commands: &mut Commands,
    asset_server: &AssetServer,
    local_name: &LocalPlayerName,
    local_skin: &LocalPlayerSkin,
    stats: &PlayerStats,
) {
    // Player on the LEFT (frame unique : backward/f1.png)
    let skin_name = if local_skin.0.is_empty() {
        "default"
    } else {
        &local_skin.0
    };
    let player_texture: Handle<Image> =
        asset_server.load(format!("skin/{}/backward/f1.png", skin_name));

    commands.spawn((
        SpriteBundle {
            texture: player_texture,
            sprite: Sprite {
                custom_size: Some(Vec2::splat(250.0)),
                ..default()
            },
            transform: Transform::from_xyz(-520.0, -300.0, COMBAT_Z_ENTITIES),
            ..default()
        },
        CombatEntity,
        CombatPlayerSprite { base_x: -520.0 },
    ));

    // Player HUD (Name + HP Bar above player)
    let p_name = local_name.0.as_deref().unwrap_or("Player");
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(12.0),
                    top: Val::Percent(16.0),
                    width: Val::Px(280.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                background_color: Color::rgba(0.08, 0.08, 0.12, 0.85).into(),
                border_color: Color::rgb(0.3, 0.7, 0.9).into(),
                ..default()
            },
            CombatEntity,
        ))
        .with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                format!("Lv.{} {}", stats.level.max(1), p_name),
                TextStyle {
                    font_size: 20.0,
                    color: Color::rgb(0.9, 0.9, 1.0),
                    ..default()
                },
            ));

            // HP track
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Px(16.0),
                        margin: UiRect::top(Val::Px(6.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    background_color: Color::rgb(0.15, 0.15, 0.18).into(),
                    border_color: Color::rgb(0.4, 0.4, 0.5).into(),
                    ..default()
                })
                .with_children(|bar| {
                    let pct = if stats.max_hp > 0 {
                        (stats.hp as f32 / stats.max_hp as f32).clamp(0.0, 1.0) * 100.0
                    } else {
                        100.0
                    };
                    bar.spawn((
                        NodeBundle {
                            style: Style {
                                width: Val::Percent(pct),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            background_color: Color::rgb(0.2, 0.85, 0.35).into(),
                            ..default()
                        },
                        PlayerHpBarFill,
                    ));
                });

            // HP Text
            parent.spawn((
                TextBundle::from_section(
                    format!("HP : {} / {}", stats.hp, stats.max_hp),
                    TextStyle {
                        font_size: 14.0,
                        color: Color::rgb(0.8, 0.8, 0.85),
                        ..default()
                    },
                )
                .with_style(Style {
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                }),
                PlayerHpText,
            ));
        });
}

/// enemy on the right (sprite, name tag and hp bar).
fn spawn_arena_enemy(
    commands: &mut Commands,
    asset_server: &AssetServer,
    combat: &CombatResource,
) {
    // Enemy on the RIGHT
    let enemy_texture: Handle<Image> =
        asset_server.load(format!("mob/{}.png", combat.combat_sprite));
    commands.spawn((
        SpriteBundle {
            texture: enemy_texture,
            sprite: Sprite {
                custom_size: Some(Vec2::splat(320.0)),
                flip_x: false,
                ..default()
            },
            transform: Transform::from_xyz(520.0, -310.0, COMBAT_Z_ENTITIES),
            ..default()
        },
        CombatEnemyVisual {
            timer: Timer::from_seconds(0.18, TimerMode::Repeating),
            frame_index: 0,
            frames: Vec::new(),
        },
        CombatEnemySpritePos { base_x: 520.0 },
        CombatEntity,
    ));

    // Enemy HUD (Name + HP Bar above enemy)
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    right: Val::Percent(12.0),
                    top: Val::Percent(16.0),
                    width: Val::Px(280.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                background_color: Color::rgba(0.08, 0.08, 0.12, 0.85).into(),
                border_color: Color::rgb(0.9, 0.3, 0.3).into(),
                ..default()
            },
            CombatEntity,
        ))
        .with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                &combat.enemy_name,
                TextStyle {
                    font_size: 20.0,
                    color: Color::rgb(1.0, 0.85, 0.85),
                    ..default()
                },
            ));

            // HP track
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Px(16.0),
                        margin: UiRect::top(Val::Px(6.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    background_color: Color::rgb(0.15, 0.15, 0.18).into(),
                    border_color: Color::rgb(0.4, 0.4, 0.5).into(),
                    ..default()
                })
                .with_children(|bar| {
                    let pct = if combat.enemy_max_hp > 0 {
                        (combat.enemy_hp as f32 / combat.enemy_max_hp as f32).clamp(0.0, 1.0)
                            * 100.0
                    } else {
                        100.0
                    };
                    bar.spawn((
                        NodeBundle {
                            style: Style {
                                width: Val::Percent(pct),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            background_color: Color::rgb(0.95, 0.25, 0.25).into(),
                            ..default()
                        },
                        EnemyHpBarFill,
                    ));
                });

            // HP Text
            parent.spawn((
                TextBundle::from_section(
                    format!("HP : {} / {}", combat.enemy_hp, combat.enemy_max_hp),
                    TextStyle {
                        font_size: 14.0,
                        color: Color::rgb(0.85, 0.8, 0.8),
                        ..default()
                    },
                )
                .with_style(Style {
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                }),
                EnemyHpText,
            ));
        });
}

/// action menu with its 4 buttons and the cursor.
fn spawn_arena_action_menu(
    commands: &mut Commands,
    asset_server: &AssetServer,
    combat: &CombatResource,
) {
    // Combat Actions Menu (LEFT side towards middle of screen)
    // slots — Main shows 3 (slot 3 hidden), AttackSub shows all 4
    let btn_textures: [Handle<Image>; 4] = [
        asset_server.load("CombatUI_Attaquer.png"), // 0: Attack  / Physical
        asset_server.load("CombatUI_Objet.png"),    // 1: Item    / Heavy Phys
        asset_server.load("CombatUI_Fuir.png"),     // 2: Flee    / Magical
        asset_server.load("CombatUI_Attaquer.png"), // 3: (hidden in Main) / Class1 Magic
    ];

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(6.0),
                    top: Val::Percent(38.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexStart,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                ..default()
            },
            CombatEntity,
        ))
        .with_children(|menu| {
            for (i, texture) in btn_textures.into_iter().enumerate() {
                let is_selected = combat.selected_action == i;
                let initial_w = if is_selected { 210.0 } else { 195.0 };
                let initial_h = if is_selected { 72.0 } else { 66.8 };
                // Slot 3 is only visible in AttackSub menu
                let slot_display = if i == 3 { Display::None } else { Display::Flex };

                menu.spawn((
                    NodeBundle {
                        style: Style {
                            display: slot_display,
                            width: Val::Px(240.0),
                            height: Val::Px(74.0),
                            justify_content: JustifyContent::FlexStart,
                            align_items: AlignItems::Center,
                            position_type: PositionType::Relative,
                            ..default()
                        },
                        ..default()
                    },
                    CombatBtnSlot(i),
                ))
                .with_children(|slot| {
                    slot.spawn((
                        ImageBundle {
                            image: UiImage::new(texture),
                            style: Style {
                                width: Val::Px(initial_w),
                                height: Val::Px(initial_h),
                                ..default()
                            },
                            ..default()
                        },
                        CombatButtonVisual {
                            index: i,
                            current_width: initial_w,
                            current_height: initial_h,
                        },
                    ));
                });
            }

            // Animated golden arrow cursor on the left of the selected button
            let cursor_texture: Handle<Image> = asset_server.load("curseur.png");
            let initial_cursor_top = match combat.selected_action {
                0 => 20.0,
                1 => 100.0,
                2 => 180.0,
                _ => 260.0,
            };

            menu.spawn((
                ImageBundle {
                    image: UiImage::new(cursor_texture),
                    style: Style {
                        position_type: PositionType::Absolute,
                        left: Val::Px(-44.0),
                        top: Val::Px(initial_cursor_top),
                        width: Val::Px(34.0),
                        height: Val::Px(34.0),
                        ..default()
                    },
                    ..default()
                },
                CombatCursorVisual {
                    current_top: initial_cursor_top,
                },
            ));
        });
}

/// banner at the bottom that shows what happens.
fn spawn_arena_log_banner(
    commands: &mut Commands,
    combat: &CombatResource,
) {
    // Combat Log Banner (status / feedback at the bottom of the screen)
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    bottom: Val::Percent(6.0),
                    left: Val::Percent(22.0),
                    right: Val::Percent(22.0),
                    height: Val::Px(48.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(Val::Px(20.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                background_color: Color::rgba(0.04, 0.04, 0.08, 0.92).into(),
                border_color: Color::rgba(0.85, 0.75, 0.35, 0.85).into(),
                ..default()
            },
            CombatEntity,
        ))
        .with_children(|parent| {
            // Message line
            parent.spawn((
                TextBundle::from_section(
                    &combat.combat_message,
                    TextStyle {
                        font_size: 18.0,
                        color: Color::rgb(1.0, 0.92, 0.65),
                        ..default()
                    },
                )
                .with_text_justify(JustifyText::Center),
                CombatMessageText,
            ));
        });
}

pub(crate) fn spawn_combat_arena(
    commands: &mut Commands,
    asset_server: &AssetServer,
    local_name: &LocalPlayerName,
    local_skin: &LocalPlayerSkin,
    stats: &PlayerStats,
    combat: &CombatResource,
) {
    spawn_arena_background(commands, asset_server, combat);
    spawn_arena_player(commands, asset_server, local_name, local_skin, stats);
    spawn_arena_enemy(commands, asset_server, combat);
    spawn_arena_action_menu(commands, asset_server, combat);
    spawn_arena_log_banner(commands, combat);
}
