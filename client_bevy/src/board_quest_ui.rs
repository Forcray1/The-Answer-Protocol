// client_bevy/src/board_quest_ui.rs
//
// Board Quest UI — displays the 6 kill quests from the notice board
// and lets the player accept or abandon a quest.
//
// Triggered by the server event `S: EVT BOARD_QUESTS <wire_data>`.
// Wire format per slot (pipe-separated):
//   slot:mob_id:mob_name:kill_count:gold_reward:status:taker:kills_done

use bevy::prelude::*;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::AppState;

pub struct BoardQuestPlugin;

impl Plugin for BoardQuestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BoardQuestState>()
            .add_systems(
                Update,
                (
                    handle_board_quest_event,
                    render_board_quest_window,
                    handle_board_quest_input,
                )
                .run_if(in_state(AppState::InGame)),
            );
    }
}

// ── Data ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BoardSlot {
    pub slot_id: usize,
    pub mob_id: String,
    pub mob_name: String,
    pub kill_count: u32,
    pub gold_reward: i32,
    pub is_taken: bool,
    pub taker: String,
    pub kills_done: u32,
}

#[derive(Resource, Default)]
pub struct BoardQuestState {
    pub open: bool,
    pub slots: Vec<BoardSlot>,
    /// Which slot the cursor is on (0-based).
    pub cursor: usize,
    /// The username of the local player (filled by listening for the connected event).
    pub local_player: String,
    /// Slot index currently held by the local player (None if none).
    pub my_slot: Option<usize>,
}

// ── Components ────────────────────────────────────────────────────────────────

#[derive(Component)]
struct BoardQuestRoot;

#[derive(Component)]
struct BoardSlotRow(usize);

// ── Systems ──────────────────────────────────────────────────────────────────

fn handle_board_quest_event(
    mut events: EventReader<ServerMessageEvent>,
    mut bq: ResMut<BoardQuestState>,
) {
    for ev in events.read() {
        // S: OK connected … name=<username> …
        if let Some(rest) = ev.0.strip_prefix("S: OK connected ") {
            for part in rest.split_whitespace() {
                if let Some(name) = part.strip_prefix("name=") {
                    bq.local_player = name.to_string();
                }
            }
        }

        // S: EVT BOARD_QUESTS <slot1>|<slot2>|…
        if let Some(rest) = ev.0.strip_prefix("S: EVT BOARD_QUESTS ") {
            bq.slots.clear();
            bq.my_slot = None;
            for raw in rest.trim().split('|') {
                let parts: Vec<&str> = raw.split(':').collect();
                if parts.len() < 8 { continue; }
                let slot_id: usize = parts[0].parse::<usize>().unwrap_or(1).saturating_sub(1);
                let mob_id    = parts[1].to_string();
                let mob_name  = parts[2].replace('_', " ");
                let kill_count: u32 = parts[3].parse().unwrap_or(1);
                let gold_reward: i32 = parts[4].parse().unwrap_or(0);
                let is_taken  = parts[5] == "taken";
                let taker     = parts[6].to_string();
                let kills_done: u32 = parts[7].parse().unwrap_or(0);

                if is_taken && taker == bq.local_player {
                    bq.my_slot = Some(slot_id);
                }
                bq.slots.push(BoardSlot { slot_id, mob_id, mob_name, kill_count, gold_reward, is_taken, taker, kills_done });
            }
            bq.open = true;
            if bq.cursor >= bq.slots.len() { bq.cursor = 0; }
        }
    }
}

fn render_board_quest_window(
    mut commands: Commands,
    bq: Res<BoardQuestState>,
    existing: Query<Entity, With<BoardQuestRoot>>,
) {
    if !bq.is_changed() { return; }

    // Despawn old window.
    for e in &existing { commands.entity(e).despawn_recursive(); }

    if !bq.open || bq.slots.is_empty() { return; }

    let root = commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                left:  Val::Percent(15.0),
                top:   Val::Percent(10.0),
                width: Val::Percent(70.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            background_color: Color::rgba(0.05, 0.05, 0.08, 0.95).into(),
            z_index: ZIndex::Global(8000),
            ..default()
        },
        BoardQuestRoot,
    )).id();

    // Title
    commands.entity(root).with_children(|parent| {
        parent.spawn(TextBundle::from_section(
            "── Notice Board ──",
            TextStyle { font_size: 26.0, color: Color::rgb(1.0, 0.85, 0.3), ..default() },
        ));
        parent.spawn(TextBundle::from_section(
            "↑↓ navigate  |  ENTER = take/abandon  |  ESC = close",
            TextStyle { font_size: 15.0, color: Color::GRAY, ..default() },
        ));

        // Slots
        for slot in &bq.slots {
            let is_cursor = slot.slot_id == bq.cursor;
            let is_mine   = bq.my_slot == Some(slot.slot_id);

            let (row_color, text_color) = if slot.is_taken && !is_mine {
                // Taken by someone else — greyed out
                (Color::rgba(0.08, 0.08, 0.08, 0.6), Color::GRAY)
            } else if is_mine {
                (Color::rgba(0.15, 0.30, 0.15, 0.9), Color::rgb(0.5, 1.0, 0.5))
            } else if is_cursor {
                (Color::rgba(0.20, 0.20, 0.35, 0.9), Color::WHITE)
            } else {
                (Color::rgba(0.10, 0.10, 0.15, 0.8), Color::rgb(0.85, 0.85, 0.85))
            };

            let label = if is_mine {
                format!(
                    "[{}] Kill {} {} — {} gold  ({}/{} done) [YOUR QUEST]",
                    slot.slot_id + 1, slot.kill_count, slot.mob_name,
                    slot.gold_reward, slot.kills_done, slot.kill_count
                )
            } else if slot.is_taken {
                format!(
                    "[{}] Kill {} {} — {} gold  (taken by {})",
                    slot.slot_id + 1, slot.kill_count, slot.mob_name,
                    slot.gold_reward, slot.taker
                )
            } else {
                format!(
                    "[{}] Kill {} {} — {} gold",
                    slot.slot_id + 1, slot.kill_count, slot.mob_name, slot.gold_reward
                )
            };

            let cursor_prefix = if is_cursor { "▶ " } else { "  " };
            let full_label = format!("{}{}", cursor_prefix, label);

            parent.spawn((
                NodeBundle {
                    style: Style {
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                        ..default()
                    },
                    background_color: row_color.into(),
                    ..default()
                },
                BoardSlotRow(slot.slot_id),
            )).with_children(|row| {
                row.spawn(TextBundle::from_section(
                    full_label,
                    TextStyle { font_size: 18.0, color: text_color, ..default() },
                ));
            });
        }
    });
}

fn handle_board_quest_input(
    input: Res<ButtonInput<KeyCode>>,
    mut bq: ResMut<BoardQuestState>,
    sender: Option<Res<NetworkSender>>,
) {
    if !bq.open { return; }

    let Some(sender) = sender else { return };

    if input.just_pressed(KeyCode::Escape) {
        bq.open = false;
        bq.set_changed();
        return;
    }

    if input.just_pressed(KeyCode::ArrowUp) {
        if bq.cursor > 0 { bq.cursor -= 1; } else { bq.cursor = bq.slots.len().saturating_sub(1); }
        bq.set_changed();
    }
    if input.just_pressed(KeyCode::ArrowDown) {
        bq.cursor = (bq.cursor + 1) % bq.slots.len().max(1);
        bq.set_changed();
    }

    if input.just_pressed(KeyCode::Enter) || input.just_pressed(KeyCode::NumpadEnter) {
        let slot_id = bq.cursor;

        if bq.my_slot == Some(slot_id) {
            // Player owns this slot → abandon
            let _ = sender.0.send("BOARD_ABANDON\n".to_string());
        } else {
            // Try to take
            let slot_available = bq.slots.get(slot_id).map_or(false, |s| !s.is_taken);
            if slot_available {
                let _ = sender.0.send(format!("BOARD_TAKE {}\n", slot_id + 1));
            }
        }
    }
}
