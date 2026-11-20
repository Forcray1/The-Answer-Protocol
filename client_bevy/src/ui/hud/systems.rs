use bevy::prelude::*;
use crate::AppState;
use crate::combat::CombatResource;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::LocalPlayerName;
use crate::ui::chat::ChatHistory;
use crate::ui::hud::components::{HpBarFill, HpText, HudUiRoot, PlayerStats, XpBarFill, XpText};
use crate::ui::inventory::{InventoryState, SelectedItem};
use crate::ui::quest::{QuestState, SelectedQuest};

/// Back to the login screen: forget the previous session's inventory, quests, stats and chat.
pub(crate) fn reset_ui_data_on_logout(
    mut inventory: ResMut<InventoryState>,
    mut selected_item: ResMut<SelectedItem>,
    mut quests: ResMut<QuestState>,
    mut selected_quest: ResMut<SelectedQuest>,
    mut stats: ResMut<PlayerStats>,
    mut history: ResMut<ChatHistory>,
) {
    *inventory = InventoryState::default();
    *selected_item = SelectedItem::default();
    *quests = QuestState::default();
    *selected_quest = SelectedQuest::default();
    *stats = PlayerStats::default();
    *history = ChatHistory::default();
}

pub(crate) fn show_hud(mut q: Query<&mut Visibility, With<HudUiRoot>>, combat: Option<Res<CombatResource>>) {
    let in_combat = combat.as_ref().map_or(false, |c| c.in_combat);
    for mut vis in q.iter_mut() {
        *vis = if in_combat {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}

pub(crate) fn hide_hud(mut q: Query<&mut Visibility, With<HudUiRoot>>) {
    for mut vis in q.iter_mut() {
        *vis = Visibility::Hidden;
    }
}

pub(crate) fn update_hud_combat_visibility(
    combat: Option<Res<CombatResource>>,
    mut q: Query<&mut Visibility, With<HudUiRoot>>,
) {
    let Some(combat) = combat else { return };
    if combat.is_changed() {
        for mut vis in q.iter_mut() {
            *vis = if combat.in_combat {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
        }
    }
}

pub(crate) fn handle_player_stats(
    mut events: EventReader<crate::net::ServerMessageEvent>,
    mut stats: ResMut<PlayerStats>,
    mut hp_fill_q: Query<&mut Style, (With<HpBarFill>, Without<XpBarFill>)>,
    mut hp_text_q: Query<&mut Text, (With<HpText>, Without<XpText>)>,
    mut xp_fill_q: Query<&mut Style, (With<XpBarFill>, Without<HpBarFill>)>,
    mut xp_text_q: Query<&mut Text, (With<XpText>, Without<HpText>)>,
) {
    for ev in events.read() {
        if let Some(data) = ev.0.strip_prefix("S: EVT PLAYER_STATS ") {
            let parts: Vec<&str> = data.trim().split_whitespace().collect();
            if parts.len() >= 5 {
                if let (Ok(hp), Ok(max_hp), Ok(xp), Ok(max_xp), Ok(lvl)) = (
                    parts[0].parse::<i32>(),
                    parts[1].parse::<i32>(),
                    parts[2].parse::<i32>(),
                    parts[3].parse::<i32>(),
                    parts[4].parse::<i32>(),
                ) {
                    stats.hp = hp;
                    stats.max_hp = max_hp;
                    stats.xp = xp;
                    stats.max_xp = max_xp;
                    stats.level = lvl;

                    // Update HP UI
                    let hp_pct = if max_hp > 0 {
                        (hp as f32 / max_hp as f32).clamp(0.0, 1.0) * 100.0
                    } else {
                        0.0
                    };
                    if let Ok(mut style) = hp_fill_q.get_single_mut() {
                        style.width = Val::Percent(hp_pct);
                    }
                    if let Ok(mut text) = hp_text_q.get_single_mut() {
                        text.sections[0].value = format!("{}/{}", hp, max_hp);
                    }

                    // Update XP UI
                    let xp_pct = if max_xp > 0 {
                        (xp as f32 / max_xp as f32).clamp(0.0, 1.0) * 100.0
                    } else {
                        0.0
                    };
                    if let Ok(mut style) = xp_fill_q.get_single_mut() {
                        style.width = Val::Percent(xp_pct);
                    }
                    if let Ok(mut text) = xp_text_q.get_single_mut() {
                        text.sections[0].value = format!("Lvl {}  {}/{}", lvl, xp, max_xp);
                    }
                }
            }
        }
    }
}
