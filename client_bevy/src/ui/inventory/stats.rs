use bevy::prelude::*;
use crate::net::NetworkSender;
use crate::ui::inventory::components::{InventoryGoldText, InventoryState, StatPointsText, StatResetBtn, StatUpBtn, StatValueText};

pub(crate) fn handle_char_stats(
    mut events: EventReader<crate::net::ServerMessageEvent>,
    mut stat_texts: Query<(&mut Text, &StatValueText)>,
    mut sp_text_q: Query<&mut Text, (With<StatPointsText>, Without<StatValueText>)>,
    mut statup_btn_q: Query<&mut Visibility, With<StatUpBtn>>,
) {
    for ev in events.read() {
        if let Some(data) = ev.0.strip_prefix("S: EVT CHAR_STATS ") {
            let parts: Vec<&str> = data.split_whitespace().collect();
            let values: Vec<i32> = parts.iter().filter_map(|p| p.parse().ok()).collect();
            if values.len() >= 5 {
                for (mut text, stat_marker) in stat_texts.iter_mut() {
                    if stat_marker.0 < values.len() {
                        text.sections[0].value = values[stat_marker.0].to_string();
                    }
                }
            }
            if values.len() >= 6 {
                let points = values[5];
                if let Ok(mut t) = sp_text_q.get_single_mut() {
                    t.sections[0].value = points.to_string();
                }
                // Show/hide + buttons based on available points
                let vis = if points > 0 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                for mut v in statup_btn_q.iter_mut() {
                    *v = vis;
                }
            }
        }
    }
}

pub(crate) fn handle_gold_event(
    mut events: EventReader<crate::net::ServerMessageEvent>,
    mut inv_state: ResMut<InventoryState>,
    mut gold_q: Query<&mut Text, With<InventoryGoldText>>,
) {
    for ev in events.read() {
        if let Some(rest) = ev.0.strip_prefix("S: EVT GOLD ") && let Ok(gold) = rest.trim().parse::<i32>() {
            inv_state.player_gold = gold;
            if let Ok(mut t) = gold_q.get_single_mut() {
                t.sections[0].value = gold.to_string();
            }
        }
    }
}

pub(crate) fn handle_stat_points_event(
    mut events: EventReader<crate::net::ServerMessageEvent>,
    mut sp_text_q: Query<&mut Text, With<StatPointsText>>,
    mut statup_btn_q: Query<&mut Visibility, With<StatUpBtn>>,
) {
    for ev in events.read() {
        if let Some(rest) = ev.0.strip_prefix("S: EVT STAT_POINTS ") && let Ok(points) = rest.trim().parse::<i32>() {
            if let Ok(mut t) = sp_text_q.get_single_mut() {
                t.sections[0].value = points.to_string();
            }
            let vis = if points > 0 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            for mut v in statup_btn_q.iter_mut() {
                *v = vis;
            }
        }
    }
}

// Named query types: they keep the system signatures short and readable
// (a long inline Query type triggers clippy::type_complexity).
type StatUpButtonQuery<'w, 's> = Query<'w, 's, (&'static Interaction, &'static StatUpBtn, &'static mut BackgroundColor), (Changed<Interaction>, With<Button>)>;

pub(crate) fn handle_statup_click(
    mut btn_q: StatUpButtonQuery,
    sender: Option<Res<NetworkSender>>,
) {
    let Some(sender) = sender else { return };
    for (interaction, stat_btn, mut bg) in &mut btn_q {
        match *interaction {
            Interaction::Pressed => {
                let _ = sender.0.try_send(format!("STATUP {}\n", stat_btn.0));
            }
            Interaction::Hovered => *bg = Color::rgba(0.2, 0.5, 0.2, 0.9).into(),
            Interaction::None => *bg = Color::rgba(0.2, 0.5, 0.2, 0.0).into(),
        }
    }
}

type StatResetButtonQuery<'w, 's> = Query<'w, 's, (&'static Interaction, &'static mut BackgroundColor), (Changed<Interaction>, With<StatResetBtn>)>;

pub(crate) fn handle_statreset_click(
    mut btn_q: StatResetButtonQuery,
    sender: Option<Res<NetworkSender>>,
) {
    let Some(sender) = sender else { return };
    for (interaction, mut bg) in &mut btn_q {
        match *interaction {
            Interaction::Pressed => {
                let _ = sender.0.try_send("STATRESET\n".to_string());
            }
            Interaction::Hovered => *bg = Color::rgba(0.7, 0.2, 0.15, 0.9).into(),
            Interaction::None => *bg = Color::rgba(0.5, 0.15, 0.1, 0.5).into(),
        }
    }
}
