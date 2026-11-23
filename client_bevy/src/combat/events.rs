use crate::net::ServerMessageEvent;
use crate::player::{LocalPlayerName, LocalPlayerSkin};
use crate::ui::PlayerStats;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use crate::combat::arena::spawn_combat_arena;
use crate::combat::components::{CombatEntity, CombatOverlay};
use crate::combat::state::{ChargePhase, CombatResource, CombatTransitionPhase, FADE_SPEED, MenuLevel, TurnSequence};

pub(crate) fn setup_combat_overlay(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut combat: ResMut<CombatResource>,
) {
    // Preload all 5 backgrounds immediately to eliminate loading time
    for i in 1..=5 {
        combat
            .bg_textures
            .push(asset_server.load(format!("maps/FightMap/FightBackground{}.webp", i)));
    }
    // Preload combat buttons and cursor
    let _ = asset_server.load::<Image>("CombatUI_Attaquer.png");
    let _ = asset_server.load::<Image>("CombatUI_Objet.png");
    let _ = asset_server.load::<Image>("CombatUI_Fuir.png");
    let _ = asset_server.load::<Image>("CombatUI_Physic.png");
    let _ = asset_server.load::<Image>("CombatUI_Heavy.png");
    let _ = asset_server.load::<Image>("CombatUI_Magic.png");
    let _ = asset_server.load::<Image>("CombatUI_HeavyMagic.png");
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

pub(crate) fn listen_combat_events(
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
                combat.skip_next_player_turn = false;
                combat.enemy_id = parts[0].to_string();
                combat.combat_sprite = parts[1]
                    .strip_suffix(".png")
                    .unwrap_or(parts[1])
                    .to_string();
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
                combat.enemy_hp = parts[0].parse().unwrap_or(combat.enemy_hp);
                combat.enemy_max_hp = parts[1].parse().unwrap_or(combat.enemy_max_hp);
                combat.pending_dealt = parts[2].parse().unwrap_or(0);
                combat.pending_taken = parts[3].parse().unwrap_or(0);
                // The sequence driver will pick these up when the player anim finishes
            }
        }

        if let Some(rest) = line.strip_prefix("S: EVT COMBAT STUN_APPLIED ") {
            let turns = rest.trim().parse::<u32>().unwrap_or(0);
            combat.skip_next_player_turn = turns > 0;
        }

        if let Some(rest) = line.strip_prefix("S: EVT COMBAT STUN ") {
            let player_name = rest.trim().replace('_', " ");
            combat.combat_message =
                format!("{} is stunned by his previous attack and can't moove.", player_name);
            combat.message_timer = 1.8;
        }

        if let Some(rest) = line.strip_prefix("S: EVT COMBAT STUN_HIT ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 3 {
                combat.enemy_hp = parts[0].parse().unwrap_or(combat.enemy_hp);
                combat.enemy_max_hp = parts[1].parse().unwrap_or(combat.enemy_max_hp);
                combat.pending_taken = parts[2].parse().unwrap_or(0);
            }
        }

        // Result of the "Item" button: only the server knows if a potion was available.
        if combat.in_combat {
            if line.starts_with("S: ERR item_not_in_inventory") {
                combat.combat_message = "You don't have any Healing Potion!".to_string();
                combat.message_timer = 1.5;
            } else if line.starts_with("S: ERR Your HP is already full") {
                combat.combat_message = "Your HP is already full!".to_string();
                combat.message_timer = 1.5;
            } else if let Some(rest) = line.strip_prefix("S: OK Used ") {
                let summary = rest.split(" (HP:").next().unwrap_or(rest);
                combat.combat_message = format!("Used {}", summary);
                combat.message_timer = 1.5;
            }
        }

        // Extract damage numbers when defeating a monster (server sends OK line without EVT COMBAT HIT)
        if let Some(rest) = line.strip_prefix("S: OK You dealt ") {
            if let Some(dmg_str) = rest.split_whitespace().next() && let Ok(dmg) = dmg_str.parse::<i32>() {
                combat.pending_dealt = dmg;
            }
        } else if let Some(rest) = line.strip_prefix("S: OK Magic deals ") {
            if let Some(token) = rest.split_whitespace().next() {
                let clean = token.trim_end_matches('.');
                if let Ok(dmg) = clean.parse::<i32>() {
                    combat.pending_dealt = dmg;
                }
            }
        }

        // S: EVT COMBAT END <reason>
        if let Some(rest) = line.strip_prefix("S: EVT COMBAT END ") {
            let reason = rest.trim().to_string();
            combat.pending_end = Some(reason.clone());
            if reason == "victory" {
                combat.enemy_hp = 0;
            }
            // If animating, let the sequence driver handle it;
            // if idle (flee etc.) apply immediately
            if combat.seq == TurnSequence::Idle {
                let msg = match reason.as_str() {
                    "victory" => "Victory! The enemy collapses.".to_string(),
                    "defeat" => "Defeated... You wake up at the oasis.".to_string(),
                    _ => "You fled.".to_string(),
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

/// What the combat arena needs to know about the local player, grouped to keep the
/// system's parameter list short (clippy::too_many_arguments).
#[derive(SystemParam)]
pub(crate) struct LocalPlayerInfo<'w> {
    local_name: Res<'w, LocalPlayerName>,
    local_skin: Res<'w, LocalPlayerSkin>,
    stats: Res<'w, PlayerStats>,
}

pub(crate) fn handle_combat_transition(
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    player_info: LocalPlayerInfo,
    mut combat: ResMut<CombatResource>,
    mut overlay_q: Query<(&mut BackgroundColor, &mut Style), With<CombatOverlay>>,
    combat_entities_q: Query<Entity, With<CombatEntity>>,
) {
    let LocalPlayerInfo {
        local_name,
        local_skin,
        stats,
    } = player_info;
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
