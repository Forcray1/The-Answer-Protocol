use crate::ui::PlayerStats;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use crate::combat::components::{CombatBtnSlot, CombatButtonVisual, CombatEnemySpritePos, CombatEnemyVisual, CombatMessageText, CombatPlayerSprite, EnemyHpBarFill, EnemyHpText, PlayerHpBarFill, PlayerHpText};
use crate::combat::state::{ChargePhase, CombatResource, MenuLevel, TurnSequence};

pub(crate) fn animate_combat_entities(
    time: Res<Time>,
    images: Res<Assets<Image>>,
    mut enemy_q: Query<(&mut CombatEnemyVisual, &mut Sprite, &Handle<Image>)>,
) {
    let dt = time.delta();

    // Enemy animation loop (spritesheet slicing if 2x3, or static)
    for (mut anim, mut sprite, tex_handle) in &mut enemy_q {
        if anim.frames.is_empty() {
            let Some(img) = images.get(tex_handle) else {
                continue;
            };
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
                anim.frames
                    .push(Rect::from_corners(Vec2::ZERO, Vec2::new(w, h)));
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

// Named query types: they keep the system signatures short and readable
// (a long inline Query type triggers clippy::type_complexity).
type EnemyHpTextQuery<'w, 's> = Query<'w, 's, &'static mut Text, ( With<EnemyHpText>, Without<PlayerHpText>, Without<CombatMessageText>, )>;
type PlayerHpTextQuery<'w, 's> = Query<'w, 's, &'static mut Text, ( With<PlayerHpText>, Without<EnemyHpText>, Without<CombatMessageText>, )>;
type CombatMessageQuery<'w, 's> = Query<'w, 's, &'static mut Text, ( With<CombatMessageText>, Without<EnemyHpText>, Without<PlayerHpText>, )>;

/// The three text widgets refreshed by `update_combat_hp_bars`, grouped to keep the
/// system's parameter list short (clippy::too_many_arguments).
#[derive(SystemParam)]
pub(crate) struct CombatHpTexts<'w, 's> {
    enemy_text_q: EnemyHpTextQuery<'w, 's>,
    player_text_q: PlayerHpTextQuery<'w, 's>,
    msg_text_q: CombatMessageQuery<'w, 's>,
}

pub(crate) fn update_combat_hp_bars(
    time: Res<Time>,
    mut combat: ResMut<CombatResource>,
    stats: Res<PlayerStats>,
    mut enemy_fill_q: Query<&mut Style, (With<EnemyHpBarFill>, Without<PlayerHpBarFill>)>,
    mut player_fill_q: Query<&mut Style, (With<PlayerHpBarFill>, Without<EnemyHpBarFill>)>,
    texts: CombatHpTexts,
) {
    let CombatHpTexts {
        mut enemy_text_q,
        mut player_text_q,
        mut msg_text_q,
    } = texts;
    if !combat.in_combat {
        return;
    }

    if combat.message_timer > 0.0 {
        combat.message_timer -= time.delta_seconds();
    }

    if let Ok(mut text) = msg_text_q.get_single_mut() && !text.sections.is_empty() {
        text.sections[0].value = combat.combat_message.clone();
    }

    let e_pct = if combat.enemy_max_hp > 0 {
        (combat.enemy_hp as f32 / combat.enemy_max_hp as f32).clamp(0.0, 1.0) * 100.0
    } else {
        0.0
    };
    if let Ok(mut style) = enemy_fill_q.get_single_mut() {
        style.width = Val::Percent(e_pct);
    }
    if let Ok(mut text) = enemy_text_q.get_single_mut() && !text.sections.is_empty() {
        text.sections[0].value =
            format!("HP : {} / {}", combat.enemy_hp.max(0), combat.enemy_max_hp);
    }

    let p_pct = if stats.max_hp > 0 {
        (stats.hp as f32 / stats.max_hp as f32).clamp(0.0, 1.0) * 100.0
    } else {
        0.0
    };
    if let Ok(mut style) = player_fill_q.get_single_mut() {
        style.width = Val::Percent(p_pct);
    }
    if let Ok(mut text) = player_text_q.get_single_mut() && !text.sections.is_empty() {
        text.sections[0].value = format!("HP : {} / {}", stats.hp.max(0), stats.max_hp);
    }
}

/// Updates button images and slot 3 visibility on menu level change.
pub(crate) fn update_combat_btn_labels(
    combat: Res<CombatResource>,
    asset_server: Res<AssetServer>,
    mut slots: Query<(&mut Style, &CombatBtnSlot)>,
    mut images: Query<(&mut UiImage, &CombatButtonVisual)>,
) {
    if !combat.is_changed() {
        return;
    }

    for (mut img, btn) in images.iter_mut() {
        img.texture = match combat.menu_level {
            MenuLevel::Main => match btn.index {
                0 => asset_server.load("CombatUI_Attaquer.png"),
                1 => asset_server.load("CombatUI_Objet.png"),
                2 => asset_server.load("CombatUI_Fuir.png"),
                _ => asset_server.load("CombatUI_Attaquer.png"),
            },
            MenuLevel::AttackSub => match btn.index {
                0 => asset_server.load("CombatUI_Physic.png"),
                1 => asset_server.load("CombatUI_Heavy.png"),
                2 => asset_server.load("CombatUI_Magic.png"),
                _ => asset_server.load("CombatUI_HeavyMagic.png"),
            },
        };
    }

    for (mut style, slot) in slots.iter_mut() {
        if slot.0 == 3 {
            style.display = if combat.menu_level == MenuLevel::AttackSub {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}

/// Animates the player sprite with a forward lunge on attack.
pub(crate) fn animate_player_charge(
    time: Res<Time>,
    mut combat: ResMut<CombatResource>,
    mut player_q: Query<(&mut Transform, &CombatPlayerSprite)>,
) {
    if combat.charge_phase == ChargePhase::Idle {
        return;
    }
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
                    if combat.pending_end.as_deref() == Some("victory") {
                        combat.enemy_hp = 0;
                    }
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
        if combat.pending_end.as_deref() == Some("victory") {
            combat.enemy_hp = 0;
        }
        // Advance sequencer: show dealt damage for 2 s, then trigger enemy
        combat.combat_message = format!("You dealt {} dmg!", combat.pending_dealt);
        combat.seq = TurnSequence::PauseAfterPlayer;
        combat.seq_timer = 2.0;
    }
}

/// Animates the enemy sprite lunging toward the player on counter-attack.
pub(crate) fn animate_enemy_charge(
    time: Res<Time>,
    mut combat: ResMut<CombatResource>,
    mut enemy_q: Query<(&mut Transform, &CombatEnemySpritePos)>,
) {
    if combat.enemy_charge_phase == ChargePhase::Idle {
        return;
    }
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
