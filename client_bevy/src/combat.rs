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
                    advance_turn_sequence,
                    animate_combat_entities,
                    animate_player_charge,
                    animate_enemy_charge,
                    update_combat_hp_bars,
                    update_combat_menu_selection,
                    update_combat_btn_labels,
                ).run_if(in_state(AppState::InGame)),
            );
    }
}

// ── Constants ──────────────────────────────────────────────────────────────

const COMBAT_Z_BG: f32 = 2000.0;
const COMBAT_Z_ENTITIES: f32 = 2010.0;
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
    /// Which menu level is shown
    pub menu_level: MenuLevel,
    /// Cursor index within the current menu
    pub selected_action: usize,
    /// Full turn sequence state (replaces raw charge_phase)
    pub seq: TurnSequence,
    pub seq_timer: f32,
    /// Damage numbers received from server (stored until displayed)
    pub pending_dealt: i32,
    pub pending_taken: i32,
    pub pending_end: Option<String>,
    /// Player charge animation
    pub charge_phase: ChargePhase,
    /// Enemy charge animation
    pub enemy_charge_phase: ChargePhase,
    /// Heavy attack blocked turns remaining
    pub heavy_blocked_turns: u32,
}

/// Sequencer for player-attack → pause → enemy-attack → pause → input.
#[derive(Default, PartialEq, Clone, Copy)]
pub enum TurnSequence {
    #[default]
    Idle,               // waiting for player input
    PlayerCharging,     // player lunge animation playing
    PauseAfterPlayer,   // 2 s pause showing "dealt X dmg"
    EnemyCharging,      // enemy lunge animation playing
    PauseAfterEnemy,    // 2 s pause showing "took Y dmg"
    Recovery,           // heavy-attack recovery (no anim, just text + timer)
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

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum ChargePhase {
    #[default]
    Idle,
    ChargingForward,
    AtPeak,
    SnapBack,
}

/// Main menu has one "Attack" entry; sub-menu has the four attack types.
#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum MenuLevel {
    #[default]
    Main,       // Attack | Item | Flee
    AttackSub,  // Physical | Heavy Phys | Magical | Class1 Magic
}

/// Marks the player sprite in the combat arena (for charge animation).
#[derive(Component)]
struct CombatPlayerSprite { base_x: f32 }

/// Marks the enemy sprite in the combat arena (for charge animation).
#[derive(Component)]
struct CombatEnemySpritePos { base_x: f32 }

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

#[derive(Component)]
struct CombatButtonVisual {
    index: usize,
    current_width: f32,
    current_height: f32,
}

#[derive(Component)]
struct CombatCursorVisual {
    current_top: f32,
}

/// Text overlay label on a combat button (updated per frame from MenuLevel).
#[derive(Component)]
struct CombatBtnLabel(usize);

/// Marks the outer NodeBundle wrapper of each button slot (for show/hide).
#[derive(Component)]
struct CombatBtnSlot(#[allow(dead_code)] usize);

// ── Systems ────────────────────────────────────────────────────────────────

fn setup_combat_overlay(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut combat: ResMut<CombatResource>,
) {
    // Preload all 5 backgrounds immediately to eliminate loading time
    for i in 1..=5 {
        combat.bg_textures.push(asset_server.load(format!("maps/FightMap/FightBackground{}.webp", i)));
    }
    // Preload combat buttons and cursor
    let _ = asset_server.load::<Image>("CombatUI_Attaquer.png");
    let _ = asset_server.load::<Image>("CombatUI_Objet.png");
    let _ = asset_server.load::<Image>("CombatUI_Fuir.png");
    let _ = asset_server.load::<Image>("curseur.png");

    // Fullscreen UI overlay at Z=9999: covers both the 2D world AND all UI
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
                combat.selected_action = 0;
                combat.menu_level = MenuLevel::Main;
                combat.seq = TurnSequence::Idle;
                combat.seq_timer = 0.0;
                combat.heavy_blocked_turns = 0;
                combat.enemy_id = parts[0].to_string();
                combat.combat_sprite = parts[1].strip_suffix(".png").unwrap_or(parts[1]).to_string();
                let _ = asset_server.load::<Image>(format!("mob/{}.png", combat.combat_sprite));
                combat.enemy_hp = parts[2].parse().unwrap_or(50);
                combat.enemy_max_hp = parts[3].parse().unwrap_or(50);
                combat.bg_index = parts[4].parse().unwrap_or(1);
                combat.enemy_name = parts[5..].join(" ").replace('_', " ");
                combat.combat_message = format!("A battle starts against {}!", combat.enemy_name);
                combat.message_timer = 2.5;
                combat.fade_alpha = 0.0;
                combat.phase = CombatTransitionPhase::FadingToBlack;
            }
        }

        // S: EVT COMBAT HIT <enemy_hp> <max_hp> <dmg_dealt> <dmg_taken>
        if let Some(rest) = line.strip_prefix("S: EVT COMBAT HIT ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 4 {
                combat.enemy_hp     = parts[0].parse().unwrap_or(combat.enemy_hp);
                combat.enemy_max_hp = parts[1].parse().unwrap_or(combat.enemy_max_hp);
                combat.pending_dealt = parts[2].parse().unwrap_or(0);
                combat.pending_taken = parts[3].parse().unwrap_or(0);
                // The sequence driver will pick these up when the player anim finishes
            }
        }

        // S: EVT COMBAT END <reason>
        if let Some(rest) = line.strip_prefix("S: EVT COMBAT END ") {
            let reason = rest.trim().to_string();
            combat.pending_end = Some(reason.clone());
            // If animating, let the sequence driver handle it;
            // if idle (flee etc.) apply immediately
            if combat.seq == TurnSequence::Idle {
                let msg = match reason.as_str() {
                    "victory" => "Victory! The enemy collapses.".to_string(),
                    "defeat"  => "Defeated... You wake up at the oasis.".to_string(),
                    _         => "You fled.".to_string(),
                };
                combat.combat_message = msg;
                combat.message_timer = 1.5;
                combat.phase = CombatTransitionPhase::ExitingToBlack;
                combat.charge_phase = ChargePhase::Idle;
                combat.enemy_charge_phase = ChargePhase::Idle;
                combat.seq = TurnSequence::Idle;
                combat.pending_end = None;
            }
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
                // Screen fully black: switch in_combat = true to hide map elements
                // and show the combat arena before the opening fade
                combat.in_combat = true;
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
            transform: Transform::from_xyz(-520.0, -300.0, COMBAT_Z_ENTITIES),
            ..default()
        },
        CombatEntity,
        CombatPlayerSprite { base_x: -520.0 },
    ));

    // Player HUD (Name + HP Bar above player)
    let p_name = local_name.0.as_deref().unwrap_or("Player");
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

    // 4. Combat Actions Menu (LEFT side towards middle of screen)
    // 4 slots — Main shows 3 (slot 3 hidden), AttackSub shows all 4
    let btn_textures: [Handle<Image>; 4] = [
        asset_server.load("CombatUI_Attaquer.png"), // 0: Attack  / Physical
        asset_server.load("CombatUI_Objet.png"),    // 1: Item    / Heavy Phys
        asset_server.load("CombatUI_Fuir.png"),     // 2: Flee    / Magical
        asset_server.load("CombatUI_Attaquer.png"), // 3: (hidden in Main) / Class1 Magic
    ];

    commands.spawn((
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
    )).with_children(|menu| {
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
            )).with_children(|slot| {
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
                // Text label overlay — empty in Main (image has baked-in text), populated in AttackSub
                slot.spawn((
                    TextBundle::from_section(
                        "",   // always starts empty; update_combat_btn_labels fills AttackSub labels
                        TextStyle { font_size: 20.0, color: Color::WHITE, ..default() },
                    ).with_style(Style {
                        position_type: PositionType::Absolute,
                        left: Val::Px(28.0),
                        top: Val::Px(18.0),
                        ..default()
                    }),
                    CombatBtnLabel(i),
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

    // 5. Combat Log Banner (status / feedback at the bottom of the screen)
    commands.spawn((
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
    )).with_children(|parent| {
        // Message line
        parent.spawn((
            TextBundle::from_section(
                &combat.combat_message,
                TextStyle {
                    font_size: 18.0,
                    color: Color::rgb(1.0, 0.92, 0.65),
                    ..default()
                },
            ).with_text_justify(JustifyText::Center),
            CombatMessageText,
        ));
    });
}

fn handle_combat_inputs(
    input: Res<ButtonInput<KeyCode>>,
    mut combat: ResMut<CombatResource>,
    sender: Res<NetworkSender>,
    console: Option<Res<crate::ui::ChatConsole>>,
) {
    if !combat.in_combat || combat.phase != CombatTransitionPhase::None { return; }
    // Block input until the full turn sequence is done
    if combat.seq != TurnSequence::Idle { return; }
    if let Some(ref c) = console {
        if c.open || c.terminal_open { return; }
    }

    let num_options: usize = match combat.menu_level {
        MenuLevel::Main      => 3, // Attack | Item | Flee
        MenuLevel::AttackSub => 4, // Physical | Heavy Phys | Magical | Class1 Magic
    };

    if input.just_pressed(KeyCode::ArrowUp) || input.just_pressed(KeyCode::ArrowLeft) {
        if combat.selected_action == 0 { combat.selected_action = num_options - 1; }
        else { combat.selected_action -= 1; }
    } else if input.just_pressed(KeyCode::ArrowDown) || input.just_pressed(KeyCode::ArrowRight) {
        if combat.selected_action >= num_options - 1 { combat.selected_action = 0; }
        else { combat.selected_action += 1; }
    }

    let validate = input.just_pressed(KeyCode::Enter)
        || input.just_pressed(KeyCode::NumpadEnter)
        || input.just_pressed(KeyCode::Space)
        || input.just_pressed(KeyCode::KeyE);

    if validate {
        match combat.menu_level {
            MenuLevel::Main => match combat.selected_action {
                0 => { // → Attack sub-menu
                    combat.menu_level = MenuLevel::AttackSub;
                    combat.selected_action = 0;
                }
                1 => { // Item — use healing potion
                    let _ = sender.0.send("USE healing_potion\n".to_string());
                    combat.combat_message = "Used a Healing Potion!".to_string();
                    combat.message_timer = 1.5;
                }
                _ => { // Flee
                    let _ = sender.0.send("FLEE\n".to_string());
                }
            },
            MenuLevel::AttackSub => match combat.selected_action {
                0 => { // Physical Attack
                    if !combat.enemy_id.is_empty() {
                        let _ = sender.0.send(format!("ATTACK {}\n", combat.enemy_id));
                        combat.combat_message = format!("You attack {}!", combat.enemy_name);
                        combat.charge_phase = ChargePhase::ChargingForward;
                        combat.seq = TurnSequence::PlayerCharging;
                    }
                    combat.menu_level = MenuLevel::Main;
                }
                1 => { // Heavy Physical (1.75×)
                    if combat.heavy_blocked_turns > 0 {
                        combat.combat_message = "Heavy attack is recharging!".to_string();
                        combat.message_timer = 1.5;
                    } else if !combat.enemy_id.is_empty() {
                        let _ = sender.0.send(format!("ATTACK_HEAVY {}\n", combat.enemy_id));
                        combat.combat_message = format!("You charge a heavy blow at {}!", combat.enemy_name);
                        combat.charge_phase = ChargePhase::ChargingForward;
                        combat.seq = TurnSequence::PlayerCharging;
                        combat.heavy_blocked_turns = 1;
                    }
                    combat.menu_level = MenuLevel::Main;
                }
                2 => { // Magical Attack
                    if !combat.enemy_id.is_empty() {
                        let _ = sender.0.send(format!("ATTACK_MAGIC {}\n", combat.enemy_id));
                        combat.combat_message = format!("You cast magic at {}!", combat.enemy_name);
                        combat.charge_phase = ChargePhase::ChargingForward;
                        combat.seq = TurnSequence::PlayerCharging;
                    }
                    combat.menu_level = MenuLevel::Main;
                }
                3 => { // Class1 Magic (Heavy Magic)
                    if combat.heavy_blocked_turns > 0 {
                        combat.combat_message = "Class magic is recharging!".to_string();
                        combat.message_timer = 1.5;
                    } else if !combat.enemy_id.is_empty() {
                        let _ = sender.0.send(format!("ATTACK_MAGIC_HEAVY {}\n", combat.enemy_id));
                        combat.combat_message = format!("You channel class magic at {}!", combat.enemy_name);
                        combat.charge_phase = ChargePhase::ChargingForward;
                        combat.seq = TurnSequence::PlayerCharging;
                        combat.heavy_blocked_turns = 1;
                    }
                    combat.menu_level = MenuLevel::Main;
                }
                _ => { combat.menu_level = MenuLevel::Main; combat.selected_action = 0; } // Back
            },
        }
    }

    if input.just_pressed(KeyCode::Escape) || input.just_pressed(KeyCode::KeyB) {
        match combat.menu_level {
            MenuLevel::Main => { let _ = sender.0.send("FLEE\n".to_string()); }
            MenuLevel::AttackSub => { combat.menu_level = MenuLevel::Main; combat.selected_action = 0; }
        }
    }
}

/// Drives the turn sequence state machine between animations and pauses.
fn advance_turn_sequence(
    time: Res<Time>,
    mut combat: ResMut<CombatResource>,
) {
    let dt = time.delta_seconds();
    match combat.seq {
        TurnSequence::PauseAfterPlayer => {
            combat.seq_timer -= dt;
            if combat.seq_timer <= 0.0 {
                // Kick off enemy lunge
                combat.enemy_charge_phase = ChargePhase::ChargingForward;
                combat.seq = TurnSequence::EnemyCharging;
            }
        }
        TurnSequence::PauseAfterEnemy => {
            combat.seq_timer -= dt;
            if combat.seq_timer <= 0.0 {
                // Decrement heavy-attack cooldown after full turn
                if combat.heavy_blocked_turns > 0 { combat.heavy_blocked_turns -= 1; }
                // Check for pending combat end (victory / defeat / flee)
                if let Some(reason) = combat.pending_end.take() {
                    let msg = match reason.as_str() {
                        "victory" => "Victory! The enemy collapses.".to_string(),
                        "defeat"  => "Defeated... You wake up at the oasis.".to_string(),
                        _         => "You fled.".to_string(),
                    };
                    combat.combat_message = msg;
                    combat.message_timer = 1.5;
                    combat.charge_phase = ChargePhase::Idle;
                    combat.enemy_charge_phase = ChargePhase::Idle;
                    combat.seq = TurnSequence::Idle;
                    combat.phase = CombatTransitionPhase::ExitingToBlack;
                    return;
                }
                combat.seq = TurnSequence::Idle;
            }
        }
        TurnSequence::Recovery => {
            combat.seq_timer -= dt;
            if combat.seq_timer <= 0.0 {
                combat.seq = TurnSequence::Idle;
            }
        }
        _ => {}
    }
}

fn update_combat_menu_selection(
    time: Res<Time>,
    combat: Res<CombatResource>,
    mut button_q: Query<(&mut Style, &mut CombatButtonVisual), Without<CombatCursorVisual>>,
    mut cursor_q: Query<(&mut Style, &mut CombatCursorVisual), Without<CombatButtonVisual>>,
) {
    if !combat.in_combat {
        return;
    }
    let dt = time.delta_seconds();
    let t = time.elapsed_seconds();

    // 1. Buttons (smooth, moderate scale-up)
    for (mut style, mut btn) in &mut button_q {
        let is_selected = btn.index == combat.selected_action;
        let target_w = if is_selected { 210.0 } else { 195.0 };
        let target_h = if is_selected { 72.0 } else { 66.8 };

        btn.current_width += (target_w - btn.current_width) * (20.0 * dt).min(1.0);
        btn.current_height += (target_h - btn.current_height) * (20.0 * dt).min(1.0);

        style.width = Val::Px(btn.current_width);
        style.height = Val::Px(btn.current_height);
    }

    // 2. Cursor (smooth slide and retro oscillation)
    let target_top = match combat.selected_action {
        0 => 20.0,
        1 => 100.0,
        2 => 180.0,
        _ => 260.0,
    };
    let bob = (t * 6.0).sin() * 3.5;
    for (mut style, mut cursor) in &mut cursor_q {
        cursor.current_top += (target_top - cursor.current_top) * (22.0 * dt).min(1.0);
        style.top = Val::Px(cursor.current_top);
        style.left = Val::Px(-44.0 + bob);
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

/// Returns the label text for each button slot based on menu level.
fn menu_label_for(level: MenuLevel, index: usize) -> &'static str {
    match level {
        MenuLevel::Main => match index {
            0 => "Attack",
            1 => "Item",
            _ => "Flee",
        },
        MenuLevel::AttackSub => match index {
            0 => "Physical",
            1 => "Heavy Phys",
            2 => "Magical",
            _ => "Class1 Magic",
        },
    }
}

/// Updates button images, text labels and slot 3 visibility on menu level change.
fn update_combat_btn_labels(
    combat: Res<CombatResource>,
    asset_server: Res<AssetServer>,
    mut labels: Query<(&mut Text, &CombatBtnLabel)>,
    mut slots: Query<(&mut Style, &CombatBtnSlot)>,
    mut images: Query<(&mut UiImage, &CombatButtonVisual)>,
) {
    if !combat.is_changed() { return; }

    // ── 1. Swap button images ────────────────────────────────────────────
    // Main menu:   each slot keeps its dedicated image (Attack / Item / Flee / hidden)
    // AttackSub:   all 4 slots use the Attack image; text labels identify them
    for (mut img, btn) in images.iter_mut() {
        img.texture = match combat.menu_level {
            MenuLevel::Main => match btn.index {
                0 => asset_server.load("CombatUI_Attaquer.png"),
                1 => asset_server.load("CombatUI_Objet.png"),
                2 => asset_server.load("CombatUI_Fuir.png"),
                _ => asset_server.load("CombatUI_Attaquer.png"),
            },
            MenuLevel::AttackSub => asset_server.load("CombatUI_Attaquer.png"),
        };
    }

    // ── 2. Show slot 3 only in AttackSub ────────────────────────────────
    for (mut style, slot) in slots.iter_mut() {
        if slot.0 == 3 {
            style.display = if combat.menu_level == MenuLevel::AttackSub {
                Display::Flex
            } else {
                Display::None
            };
        }
    }

    // ── 3. Text labels ───────────────────────────────────────────────────
    // Main:     images carry baked-in text → overlay stays empty
    // AttackSub: overlay shows the attack type name
    for (mut text, lbl) in labels.iter_mut() {
        text.sections[0].value = match combat.menu_level {
            MenuLevel::Main => String::new(),
            MenuLevel::AttackSub => menu_label_for(combat.menu_level, lbl.0).to_string(),
        };
    }
}

/// Animates the player sprite with a forward lunge on attack.
fn animate_player_charge(
    time: Res<Time>,
    mut combat: ResMut<CombatResource>,
    mut player_q: Query<(&mut Transform, &CombatPlayerSprite)>,
) {
    if combat.charge_phase == ChargePhase::Idle { return; }
    let dt = time.delta_seconds();

    let mut finished = false;
    for (mut tf, ps) in player_q.iter_mut() {
        match combat.charge_phase {
            ChargePhase::ChargingForward => {
                let target_x = ps.base_x + 260.0;
                tf.translation.x += 1800.0 * dt;
                if tf.translation.x >= target_x {
                    tf.translation.x = target_x;
                    combat.charge_phase = ChargePhase::AtPeak;
                    combat.seq_timer = 0.10;
                }
            }
            ChargePhase::AtPeak => {
                combat.seq_timer -= dt;
                if combat.seq_timer <= 0.0 {
                    combat.charge_phase = ChargePhase::SnapBack;
                }
            }
            ChargePhase::SnapBack => {
                let target_x = ps.base_x;
                tf.translation.x -= 2400.0 * dt;
                if tf.translation.x <= target_x {
                    tf.translation.x = target_x;
                    finished = true;
                }
            }
            ChargePhase::Idle => {}
        }
    }
    if finished {
        combat.charge_phase = ChargePhase::Idle;
        // Advance sequencer: show dealt damage for 2 s, then trigger enemy
        combat.combat_message = format!("You dealt {} dmg!", combat.pending_dealt);
        combat.seq = TurnSequence::PauseAfterPlayer;
        combat.seq_timer = 2.0;
    }
}

/// Animates the enemy sprite lunging toward the player on counter-attack.
fn animate_enemy_charge(
    time: Res<Time>,
    mut combat: ResMut<CombatResource>,
    mut enemy_q: Query<(&mut Transform, &CombatEnemySpritePos)>,
) {
    if combat.enemy_charge_phase == ChargePhase::Idle { return; }
    let dt = time.delta_seconds();

    let mut finished = false;
    for (mut tf, ep) in enemy_q.iter_mut() {
        match combat.enemy_charge_phase {
            ChargePhase::ChargingForward => {
                let target_x = ep.base_x - 260.0;
                tf.translation.x -= 1800.0 * dt;
                if tf.translation.x <= target_x {
                    tf.translation.x = target_x;
                    combat.enemy_charge_phase = ChargePhase::AtPeak;
                    combat.seq_timer = 0.10;
                }
            }
            ChargePhase::AtPeak => {
                combat.seq_timer -= dt;
                if combat.seq_timer <= 0.0 {
                    combat.enemy_charge_phase = ChargePhase::SnapBack;
                }
            }
            ChargePhase::SnapBack => {
                let target_x = ep.base_x;
                tf.translation.x += 2400.0 * dt;
                if tf.translation.x >= target_x {
                    tf.translation.x = target_x;
                    finished = true;
                }
            }
            ChargePhase::Idle => {}
        }
    }
    if finished {
        combat.enemy_charge_phase = ChargePhase::Idle;
        // Advance sequencer: show taken damage for 2 s
        combat.combat_message = format!("Enemy dealt {} dmg!", combat.pending_taken);
        combat.seq = TurnSequence::PauseAfterEnemy;
        combat.seq_timer = 2.0;
    }
}
