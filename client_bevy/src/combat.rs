use bevy::prelude::*;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::{LocalPlayerName, LocalPlayerSkin};
use crate::ui::PlayerStats;
use crate::AppState;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CombatResource>()
            .add_systems(Startup, setup_combat_overlay)
            .add_systems(
                Update,
                (
                    listen_combat_events,
                    handle_combat_transition,
                    handle_combat_inputs,
                    animate_combat_entities,
                    update_combat_hp_bars,
                ).run_if(in_state(AppState::InGame)),
            );
    }
}

// ── Constants ──────────────────────────────────────────────────────────────

const COMBAT_Z_BG: f32 = 500.0;
const COMBAT_Z_ENTITIES: f32 = 510.0;
const FADE_SPEED: f32 = 1.0;

// ── Resource ───────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct CombatResource {
    pub in_combat: bool,
    pub enemy_id: String,
    pub enemy_name: String,
    pub combat_sprite: String,
    pub enemy_hp: i32,
    pub enemy_max_hp: i32,
    pub bg_index: u32,
    pub phase: CombatTransitionPhase,
    pub fade_alpha: f32,
    pub combat_message: String,
    pub message_timer: f32,
    pub bg_textures: Vec<Handle<Image>>,
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum CombatTransitionPhase {
    #[default]
    None,
    FadingToBlack,
    FadingFromBlack,
    ExitingToBlack,
    ExitingFromBlack,
}

// ── Components ─────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct CombatEntity;

#[derive(Component)]
struct CombatOverlay;

#[derive(Component)]
struct CombatEnemyVisual {
    timer: Timer,
    frame_index: usize,
    frames: Vec<Rect>,
}

#[derive(Component)]
struct EnemyHpBarFill;

#[derive(Component)]
struct EnemyHpText;

#[derive(Component)]
struct PlayerHpBarFill;

#[derive(Component)]
struct PlayerHpText;

#[derive(Component)]
struct CombatMessageText;

// ── Systems ────────────────────────────────────────────────────────────────

fn setup_combat_overlay(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut combat: ResMut<CombatResource>,
) {
    // Préchargement immédiat des 5 arrière-plans pour éliminer tout temps de chargement
    for i in 1..=5 {
        combat.bg_textures.push(asset_server.load(format!("maps/FightMap/FightBackground{}.webp", i)));
    }

    // Overlay plein écran UI à Z=9999 : couvre à la fois le monde 2D ET toute l'interface UI
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::None,
                ..default()
            },
            background_color: Color::rgba(0.0, 0.0, 0.0, 0.0).into(),
            z_index: ZIndex::Global(9999),
            focus_policy: bevy::ui::FocusPolicy::Pass,
            ..default()
        },
        CombatOverlay,
    ));
}

fn listen_combat_events(
    mut events: EventReader<ServerMessageEvent>,
    asset_server: Res<AssetServer>,
    mut combat: ResMut<CombatResource>,
) {
    for ev in events.read() {
        let line = &ev.0;

        // S: EVT COMBAT START <id> <sprite> <hp> <max_hp> <bg> <name>
        if let Some(rest) = line.strip_prefix("S: EVT COMBAT START ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 6 {
                combat.in_combat = true;
                combat.enemy_id = parts[0].to_string();
                combat.combat_sprite = parts[1].strip_suffix(".png").unwrap_or(parts[1]).to_string();
                // Précharge le sprite du monstre immédiatement pour qu'il soit prêt
                let _ = asset_server.load::<Image>(format!("mob/{}.png", combat.combat_sprite));
                combat.enemy_hp = parts[2].parse().unwrap_or(50);
                combat.enemy_max_hp = parts[3].parse().unwrap_or(50);
                combat.bg_index = parts[4].parse().unwrap_or(1);
                combat.enemy_name = parts[5..].join(" ").replace('_', " ");
                combat.combat_message = format!("Un combat commence contre {} !", combat.enemy_name);
                combat.message_timer = 2.5;
                combat.fade_alpha = 0.0;
                combat.phase = CombatTransitionPhase::FadingToBlack;
                println!(
                    "[COMBAT] Début du combat contre '{}' (sprite '{}', bg {})",
                    combat.enemy_name, combat.combat_sprite, combat.bg_index
                );
            }
        }

        // S: EVT COMBAT HIT <enemy_hp> <max_hp> <dmg_dealt> <dmg_taken>
        if let Some(rest) = line.strip_prefix("S: EVT COMBAT HIT ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 4 {
                combat.enemy_hp = parts[0].parse().unwrap_or(combat.enemy_hp);
                combat.enemy_max_hp = parts[1].parse().unwrap_or(combat.enemy_max_hp);
                let dealt = parts[2];
                let taken = parts[3];
                combat.combat_message = format!("Vous infligez -{} PV ! L'ennemi riposte: -{} PV", dealt, taken);
                combat.message_timer = 2.0;
            }
        }

        // S: EVT COMBAT END <reason>
        if let Some(rest) = line.strip_prefix("S: EVT COMBAT END ") {
            let reason = rest.trim();
            combat.combat_message = match reason {
                "victory" => "Victoire ! L'ennemi s'effondre.".to_string(),
                "defeat" => "Défaite... Vous êtes K.O.".to_string(),
                _ => "Vous avez pris la fuite.".to_string(),
            };
            combat.message_timer = 1.5;
            combat.fade_alpha = 0.0;
            combat.phase = CombatTransitionPhase::ExitingToBlack;
            println!("[COMBAT] Fin du combat (raison: {})", reason);
        }
    }
}

fn handle_combat_transition(
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    local_name: Res<LocalPlayerName>,
    local_skin: Res<LocalPlayerSkin>,
    stats: Res<PlayerStats>,
    mut combat: ResMut<CombatResource>,
    mut overlay_q: Query<(&mut BackgroundColor, &mut Style), With<CombatOverlay>>,
    combat_entities_q: Query<Entity, With<CombatEntity>>,
) {
    let dt = time.delta_seconds();

    match combat.phase {
        CombatTransitionPhase::None => {}
        CombatTransitionPhase::FadingToBlack => {
            combat.fade_alpha += FADE_SPEED * dt;
            if combat.fade_alpha >= 1.0 {
                combat.fade_alpha = 1.0;
                spawn_combat_arena(
                    &mut commands,
                    &asset_server,
                    &local_name,
                    &local_skin,
                    &stats,
                    &combat,
                );
                combat.phase = CombatTransitionPhase::FadingFromBlack;
            }
        }
        CombatTransitionPhase::FadingFromBlack => {
            combat.fade_alpha -= FADE_SPEED * dt;
            if combat.fade_alpha <= 0.0 {
                combat.fade_alpha = 0.0;
                combat.phase = CombatTransitionPhase::None;
            }
        }
        CombatTransitionPhase::ExitingToBlack => {
            combat.fade_alpha += FADE_SPEED * dt;
            if combat.fade_alpha >= 1.0 {
                combat.fade_alpha = 1.0;
                for entity in &combat_entities_q {
                    commands.entity(entity).despawn_recursive();
                }
                combat.in_combat = false;
                combat.phase = CombatTransitionPhase::ExitingFromBlack;
            }
        }
        CombatTransitionPhase::ExitingFromBlack => {
            combat.fade_alpha -= FADE_SPEED * dt;
            if combat.fade_alpha <= 0.0 {
                combat.fade_alpha = 0.0;
                combat.phase = CombatTransitionPhase::None;
            }
        }
    }

    if let Ok((mut bg, mut style)) = overlay_q.get_single_mut() {
        if combat.fade_alpha <= 0.0 && combat.phase == CombatTransitionPhase::None {
            style.display = Display::None;
        } else {
            style.display = Display::Flex;
            *bg = Color::rgba(0.0, 0.0, 0.0, combat.fade_alpha.clamp(0.0, 1.0)).into();
        }
    }
}

fn spawn_combat_arena(
    commands: &mut Commands,
    asset_server: &AssetServer,
    local_name: &LocalPlayerName,
    local_skin: &LocalPlayerSkin,
    stats: &PlayerStats,
    combat: &CombatResource,
) {
    let bg_texture: Handle<Image> = if (combat.bg_index as usize) <= combat.bg_textures.len() && combat.bg_index > 0 {
        combat.bg_textures[(combat.bg_index - 1) as usize].clone()
    } else {
        asset_server.load(format!("maps/FightMap/FightBackground{}.webp", combat.bg_index))
    };

    // 1. Background image (2560 x 1440)
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

    // 2. Player on the LEFT (frame unique : backward/f1.png)
    let skin_name = if local_skin.0.is_empty() { "default" } else { &local_skin.0 };
    let player_texture: Handle<Image> = asset_server.load(format!("skin/{}/backward/f1.png", skin_name));

    commands.spawn((
        SpriteBundle {
            texture: player_texture,
            sprite: Sprite {
                custom_size: Some(Vec2::splat(250.0)),
                ..default()
            },
            transform: Transform::from_xyz(-520.0, -140.0, COMBAT_Z_ENTITIES),
            ..default()
        },
        CombatEntity,
    ));

    // Player HUD (Name + HP Bar above player)
    let p_name = local_name.0.as_deref().unwrap_or("Joueur");
    commands.spawn((
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
    )).with_children(|parent| {
        parent.spawn(TextBundle::from_section(
            format!("Lv.{} {}", stats.level.max(1), p_name),
            TextStyle {
                font_size: 20.0,
                color: Color::rgb(0.9, 0.9, 1.0),
                ..default()
            },
        ));

        // HP track
        parent.spawn(NodeBundle {
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
        }).with_children(|bar| {
            let pct = if stats.max_hp > 0 { (stats.hp as f32 / stats.max_hp as f32).clamp(0.0, 1.0) * 100.0 } else { 100.0 };
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
            ).with_style(Style {
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            }),
            PlayerHpText,
        ));
    });

    // 3. Enemy on the RIGHT
    let enemy_texture: Handle<Image> = asset_server.load(format!("mob/{}.png", combat.combat_sprite));
    commands.spawn((
        SpriteBundle {
            texture: enemy_texture,
            sprite: Sprite {
                custom_size: Some(Vec2::splat(320.0)),
                flip_x: false, // Face left towards the player
                ..default()
            },
            transform: Transform::from_xyz(520.0, -140.0, COMBAT_Z_ENTITIES),
            ..default()
        },
        CombatEnemyVisual {
            timer: Timer::from_seconds(0.18, TimerMode::Repeating),
            frame_index: 0,
            frames: Vec::new(),
        },
        CombatEntity,
    ));

    // Enemy HUD (Name + HP Bar above enemy)
    commands.spawn((
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
    )).with_children(|parent| {
        parent.spawn(TextBundle::from_section(
            &combat.enemy_name,
            TextStyle {
                font_size: 20.0,
                color: Color::rgb(1.0, 0.85, 0.85),
                ..default()
            },
        ));

        // HP track
        parent.spawn(NodeBundle {
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
        }).with_children(|bar| {
            let pct = if combat.enemy_max_hp > 0 {
                (combat.enemy_hp as f32 / combat.enemy_max_hp as f32).clamp(0.0, 1.0) * 100.0
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
            ).with_style(Style {
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            }),
            EnemyHpText,
        ));
    });

    // 4. Combat Banner (Actions + Notifications at bottom)
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                bottom: Val::Percent(8.0),
                left: Val::Percent(20.0),
                right: Val::Percent(20.0),
                height: Val::Px(85.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            background_color: Color::rgba(0.05, 0.05, 0.08, 0.90).into(),
            border_color: Color::rgb(0.8, 0.7, 0.3).into(),
            ..default()
        },
        CombatEntity,
    )).with_children(|parent| {
        // Message line
        parent.spawn((
            TextBundle::from_section(
                &combat.combat_message,
                TextStyle {
                    font_size: 18.0,
                    color: Color::rgb(1.0, 0.9, 0.5),
                    ..default()
                },
            ),
            CombatMessageText,
        ));

        // Action Hints
        parent.spawn(TextBundle::from_section(
            "[E]  Attaquer        •        [Echap]  Fuir le combat",
            TextStyle {
                font_size: 17.0,
                color: Color::rgb(0.7, 0.9, 1.0),
                ..default()
            },
        ).with_style(Style {
            margin: UiRect::top(Val::Px(8.0)),
            ..default()
        }));
    });
}

fn handle_combat_inputs(
    input: Res<ButtonInput<KeyCode>>,
    combat: Res<CombatResource>,
    sender: Res<NetworkSender>,
) {
    if !combat.in_combat || combat.phase != CombatTransitionPhase::None {
        return;
    }

    if input.just_pressed(KeyCode::KeyE) {
        if !combat.enemy_id.is_empty() {
            let _ = sender.0.send(format!("ATTACK {}\n", combat.enemy_id));
        }
    } else if input.just_pressed(KeyCode::Escape) {
        let _ = sender.0.send("FLEE\n".to_string());
    }
}

fn animate_combat_entities(
    time: Res<Time>,
    images: Res<Assets<Image>>,
    mut enemy_q: Query<(&mut CombatEnemyVisual, &mut Sprite, &Handle<Image>)>,
) {
    let dt = time.delta();

    // Enemy animation loop (spritesheet slicing if 2x3, or static)
    for (mut anim, mut sprite, tex_handle) in &mut enemy_q {
        if anim.frames.is_empty() {
            let Some(img) = images.get(tex_handle) else { continue; };
            let w = img.size().x as f32;
            let h = img.size().y as f32;

            let is_2x3 = (w * 3.0 - h * 2.0).abs() < 5.0 && w != h;
            if is_2x3 {
                let frame_w = w / 2.0;
                let frame_h = h / 3.0;
                for row in 0..3 {
                    for col in 0..2 {
                        let min = Vec2::new(col as f32 * frame_w, row as f32 * frame_h);
                        let max = Vec2::new(min.x + frame_w, min.y + frame_h);
                        anim.frames.push(Rect::from_corners(min, max));
                    }
                }
            } else {
                anim.frames.push(Rect::from_corners(Vec2::ZERO, Vec2::new(w, h)));
            }
            if let Some(first) = anim.frames.first() {
                sprite.rect = Some(*first);
            }
        }

        anim.timer.tick(dt);
        if anim.timer.just_finished() && !anim.frames.is_empty() {
            anim.frame_index = (anim.frame_index + 1) % anim.frames.len();
            if let Some(rect) = anim.frames.get(anim.frame_index) {
                sprite.rect = Some(*rect);
            }
        }
    }
}

fn update_combat_hp_bars(
    time: Res<Time>,
    mut combat: ResMut<CombatResource>,
    stats: Res<PlayerStats>,
    mut enemy_fill_q: Query<&mut Style, (With<EnemyHpBarFill>, Without<PlayerHpBarFill>)>,
    mut enemy_text_q: Query<&mut Text, (With<EnemyHpText>, Without<PlayerHpText>, Without<CombatMessageText>)>,
    mut player_fill_q: Query<&mut Style, (With<PlayerHpBarFill>, Without<EnemyHpBarFill>)>,
    mut player_text_q: Query<&mut Text, (With<PlayerHpText>, Without<EnemyHpText>, Without<CombatMessageText>)>,
    mut msg_text_q: Query<&mut Text, (With<CombatMessageText>, Without<EnemyHpText>, Without<PlayerHpText>)>,
) {
    if !combat.in_combat {
        return;
    }

    // Message timer
    if combat.message_timer > 0.0 {
        combat.message_timer -= time.delta_seconds();
    }

    if let Ok(mut text) = msg_text_q.get_single_mut() {
        if !text.sections.is_empty() {
            text.sections[0].value = combat.combat_message.clone();
        }
    }

    // Enemy HP
    let e_pct = if combat.enemy_max_hp > 0 {
        (combat.enemy_hp as f32 / combat.enemy_max_hp as f32).clamp(0.0, 1.0) * 100.0
    } else {
        0.0
    };
    if let Ok(mut style) = enemy_fill_q.get_single_mut() {
        style.width = Val::Percent(e_pct);
    }
    if let Ok(mut text) = enemy_text_q.get_single_mut() {
        if !text.sections.is_empty() {
            text.sections[0].value = format!("HP : {} / {}", combat.enemy_hp.max(0), combat.enemy_max_hp);
        }
    }

    // Player HP
    let p_pct = if stats.max_hp > 0 {
        (stats.hp as f32 / stats.max_hp as f32).clamp(0.0, 1.0) * 100.0
    } else {
        0.0
    };
    if let Ok(mut style) = player_fill_q.get_single_mut() {
        style.width = Val::Percent(p_pct);
    }
    if let Ok(mut text) = player_text_q.get_single_mut() {
        if !text.sections.is_empty() {
            text.sections[0].value = format!("HP : {} / {}", stats.hp.max(0), stats.max_hp);
        }
    }
}
