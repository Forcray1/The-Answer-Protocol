use crate::AppState;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::{LocalPlayerName, LocalPlayerSkin};
use crate::ui::PlayerStats;
use bevy::prelude::*;
use crate::combat::components::{CombatButtonVisual, CombatCursorVisual};
use crate::combat::state::{ChargePhase, CombatResource, CombatTransitionPhase, MenuLevel, TurnSequence};

pub(crate) fn handle_combat_inputs(
    input: Res<ButtonInput<KeyCode>>,
    mut combat: ResMut<CombatResource>,
    sender: Res<NetworkSender>,
    console: Option<Res<crate::ui::ChatConsole>>,
) {
    if !combat.in_combat || combat.phase != CombatTransitionPhase::None {
        return;
    }

    // Any key during damage pauses immediately skips the timer so the player is never frozen
    if combat.seq == TurnSequence::PauseAfterPlayer
        || combat.seq == TurnSequence::PauseAfterEnemy
        || combat.seq == TurnSequence::Recovery
    {
        let skip_pressed = input.just_pressed(KeyCode::Space)
            || input.just_pressed(KeyCode::Enter)
            || input.just_pressed(KeyCode::NumpadEnter)
            || input.just_pressed(KeyCode::KeyE)
            || input.just_pressed(KeyCode::Escape)
            || input.just_pressed(KeyCode::KeyB)
            || input.just_pressed(KeyCode::ArrowUp)
            || input.just_pressed(KeyCode::ArrowDown)
            || input.just_pressed(KeyCode::KeyW)
            || input.just_pressed(KeyCode::KeyS)
            || input.just_pressed(KeyCode::KeyZ);
        if skip_pressed {
            combat.seq_timer = 0.0;
        }
        return;
    }

    // If the player is stunned from a previous heavy attack, do not accept any new
    // action for the next turn. The server already enforces this as well.
    if combat.skip_next_player_turn {
        return;
    }

    // Block input while physical charge animations are playing
    if combat.seq != TurnSequence::Idle {
        return;
    }
    if let Some(ref c) = console {
        if c.open || c.terminal_open {
            return;
        }
    }

    let num_options: usize = match combat.menu_level {
        MenuLevel::Main => 3,      // Attack | Item | Flee
        MenuLevel::AttackSub => 4, // Physical | Heavy Phys | Magical | Class1 Magic
    };

    let up = input.just_pressed(KeyCode::ArrowUp)
        || input.just_pressed(KeyCode::ArrowLeft)
        || input.just_pressed(KeyCode::KeyW)
        || input.just_pressed(KeyCode::KeyZ)
        || input.just_pressed(KeyCode::KeyA)
        || input.just_pressed(KeyCode::KeyQ);

    let down = input.just_pressed(KeyCode::ArrowDown)
        || input.just_pressed(KeyCode::ArrowRight)
        || input.just_pressed(KeyCode::KeyS)
        || input.just_pressed(KeyCode::KeyD);

    if up {
        if combat.selected_action == 0 {
            combat.selected_action = num_options - 1;
        } else {
            combat.selected_action -= 1;
        }
    } else if down {
        if combat.selected_action >= num_options - 1 {
            combat.selected_action = 0;
        } else {
            combat.selected_action += 1;
        }
    }

    let validate = input.just_pressed(KeyCode::Enter)
        || input.just_pressed(KeyCode::NumpadEnter)
        || input.just_pressed(KeyCode::Space)
        || input.just_pressed(KeyCode::KeyE);

    if validate {
        match combat.menu_level {
            MenuLevel::Main => match combat.selected_action {
                0 => {
                    // -> Attack sub-menu
                    combat.menu_level = MenuLevel::AttackSub;
                    combat.selected_action = 0;
                }
                1 => {
                    // Item — use healing potion
                    let _ = sender.0.send("USE healing_potion\n".to_string());
                    combat.combat_message = "Used a Healing Potion!".to_string();
                    combat.message_timer = 1.5;
                }
                _ => {
                    // Flee
                    let _ = sender.0.send("FLEE\n".to_string());
                }
            },
            MenuLevel::AttackSub => match combat.selected_action {
                0 => {
                    // Physical Attack
                    if !combat.enemy_id.is_empty() {
                        let _ = sender.0.send(format!("ATTACK {}\n", combat.enemy_id));
                        combat.combat_message = format!("You attack {}!", combat.enemy_name);
                        combat.charge_phase = ChargePhase::ChargingForward;
                        combat.seq = TurnSequence::PlayerCharging;
                    }
                    combat.menu_level = MenuLevel::Main;
                    combat.selected_action = 0;
                }
                1 => {
                    // Heavy Physical (1.75×)
                    if combat.heavy_blocked_turns > 0 {
                        combat.combat_message = "Heavy attack is recharging!".to_string();
                        combat.message_timer = 1.5;
                    } else if !combat.enemy_id.is_empty() {
                        let _ = sender.0.send(format!("ATTACK_HEAVY {}\n", combat.enemy_id));
                        combat.combat_message =
                            format!("You charge a heavy blow at {}!", combat.enemy_name);
                        combat.charge_phase = ChargePhase::ChargingForward;
                        combat.seq = TurnSequence::PlayerCharging;
                        combat.heavy_blocked_turns = 1;
                        combat.skip_next_player_turn = true;
                    }
                    combat.menu_level = MenuLevel::Main;
                    combat.selected_action = 0;
                }
                2 => {
                    // Magical Attack
                    if !combat.enemy_id.is_empty() {
                        let _ = sender.0.send(format!("ATTACK_MAGIC {}\n", combat.enemy_id));
                        combat.combat_message = format!("You cast magic at {}!", combat.enemy_name);
                        combat.charge_phase = ChargePhase::ChargingForward;
                        combat.seq = TurnSequence::PlayerCharging;
                    }
                    combat.menu_level = MenuLevel::Main;
                    combat.selected_action = 0;
                }
                3 => {
                    // Class1 Magic (Heavy Magic)
                    if combat.heavy_blocked_turns > 0 {
                        combat.combat_message = "Class magic is recharging!".to_string();
                        combat.message_timer = 1.5;
                    } else if !combat.enemy_id.is_empty() {
                        let _ = sender
                            .0
                            .send(format!("ATTACK_MAGIC_HEAVY {}\n", combat.enemy_id));
                        combat.combat_message =
                            format!("You channel class magic at {}!", combat.enemy_name);
                        combat.charge_phase = ChargePhase::ChargingForward;
                        combat.seq = TurnSequence::PlayerCharging;
                        combat.heavy_blocked_turns = 1;
                        combat.skip_next_player_turn = true;
                    }
                    combat.menu_level = MenuLevel::Main;
                    combat.selected_action = 0;
                }
                _ => {
                    combat.menu_level = MenuLevel::Main;
                    combat.selected_action = 0;
                }
            },
        }
    }

    if input.just_pressed(KeyCode::Escape) || input.just_pressed(KeyCode::KeyB) {
        match combat.menu_level {
            MenuLevel::Main => {
                let _ = sender.0.send("FLEE\n".to_string());
            }
            MenuLevel::AttackSub => {
                combat.menu_level = MenuLevel::Main;
                combat.selected_action = 0;
            }
        }
    }
}

/// Drives the turn sequence state machine between animations and pauses.
pub(crate) fn advance_turn_sequence(
    time: Res<Time>,
    sender: Res<NetworkSender>,
    local_name: Res<LocalPlayerName>,
    mut combat: ResMut<CombatResource>,
) {
    let dt = time.delta_seconds();
    match combat.seq {
        TurnSequence::PauseAfterPlayer => {
            combat.seq_timer -= dt;
            if combat.seq_timer <= 0.0 {
                // If enemy is defeated or combat has ended, finish immediately
                if combat.enemy_hp <= 0 || combat.pending_end.is_some() {
                    if let Some(reason) = combat.pending_end.take() {
                        let msg = match reason.as_str() {
                            "victory" => "Victory! The enemy collapses.".to_string(),
                            "defeat" => "Defeated... You wake up at the oasis.".to_string(),
                            _ => "You fled.".to_string(),
                        };
                        combat.combat_message = msg;
                        combat.message_timer = 1.5;
                        combat.charge_phase = ChargePhase::Idle;
                        combat.enemy_charge_phase = ChargePhase::Idle;
                        combat.seq = TurnSequence::Idle;
                        combat.menu_level = MenuLevel::Main;
                        combat.selected_action = 0;
                        combat.phase = CombatTransitionPhase::ExitingToBlack;
                        return;
                    }
                }
                // Kick off enemy lunge
                combat.enemy_charge_phase = ChargePhase::ChargingForward;
                combat.seq = TurnSequence::EnemyCharging;
            }
        }
        TurnSequence::PauseAfterEnemy => {
            combat.seq_timer -= dt;
            if combat.seq_timer <= 0.0 {
                // Decrement heavy-attack cooldown after full turn
                if combat.heavy_blocked_turns > 0 {
                    combat.heavy_blocked_turns -= 1;
                }
                // Check for pending combat end (victory / defeat / flee)
                if let Some(reason) = combat.pending_end.take() {
                    let msg = match reason.as_str() {
                        "victory" => "Victory! The enemy collapses.".to_string(),
                        "defeat" => "Defeated... You wake up at the oasis.".to_string(),
                        _ => "You fled.".to_string(),
                    };
                    combat.combat_message = msg;
                    combat.message_timer = 1.5;
                    combat.charge_phase = ChargePhase::Idle;
                    combat.enemy_charge_phase = ChargePhase::Idle;
                    combat.seq = TurnSequence::Idle;
                    combat.menu_level = MenuLevel::Main;
                    combat.selected_action = 0;
                    combat.phase = CombatTransitionPhase::ExitingToBlack;
                    return;
                }

                if combat.skip_next_player_turn {
                    combat.skip_next_player_turn = false;
                    let who = local_name.0.as_deref().unwrap_or("Player");
                    combat.combat_message =
                        format!("{} is stunned by his previous attack and can't moove.", who);
                    combat.message_timer = 1.2;
                    combat.seq = TurnSequence::Recovery;
                    combat.seq_timer = 0.75;
                    return;
                }

                combat.seq = TurnSequence::Idle;
                combat.menu_level = MenuLevel::Main;
                combat.selected_action = 0;
            }
        }
        TurnSequence::Recovery => {
            combat.seq_timer -= dt;
            if combat.seq_timer <= 0.0 {
                if !combat.enemy_id.is_empty() {
                    let _ = sender.0.send(format!("ATTACK {}\n", combat.enemy_id));
                    combat.enemy_charge_phase = ChargePhase::ChargingForward;
                    combat.seq = TurnSequence::EnemyCharging;
                } else {
                    combat.seq = TurnSequence::Idle;
                    combat.menu_level = MenuLevel::Main;
                    combat.selected_action = 0;
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn update_combat_menu_selection(
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

    // Buttons (smooth, moderate scale-up)
    for (mut style, mut btn) in &mut button_q {
        let is_selected = btn.index == combat.selected_action;
        let target_w = if is_selected { 210.0 } else { 195.0 };
        let target_h = if is_selected { 72.0 } else { 66.8 };

        btn.current_width += (target_w - btn.current_width) * (20.0 * dt).min(1.0);
        btn.current_height += (target_h - btn.current_height) * (20.0 * dt).min(1.0);

        style.width = Val::Px(btn.current_width);
        style.height = Val::Px(btn.current_height);
    }

    // Cursor (smooth slide and retro oscillation)
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
