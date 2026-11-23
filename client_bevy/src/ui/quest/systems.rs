use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use crate::combat::CombatResource;
use crate::ui::chat::ChatConsole;
use crate::ui::inventory::InventoryState;
use crate::ui::quest::components::{QUEST_ENTRY_BG, QUEST_ENTRY_BORDER, QUEST_ENTRY_HOVER, QUEST_ENTRY_SELECTED, QUEST_ENTRY_TEXT, QuestDetailDescription, QuestDetailObjective, QuestDetailTitle, QuestEmptyText, QuestEntry, QuestEntrySlot, QuestListPanel, QuestState, QuestUiRoot, SelectedQuest};

// Named query types: they keep the system signatures short and readable
// (a long inline Query type triggers clippy::type_complexity).
type QuestDescriptionQuery<'w, 's> = Query<'w, 's, &'static mut Text, ( With<QuestDetailDescription>, Without<QuestDetailTitle>, Without<QuestDetailObjective>, )>;
type QuestObjectiveQuery<'w, 's> = Query<'w, 's, &'static mut Text, ( With<QuestDetailObjective>, Without<QuestDetailTitle>, Without<QuestDetailDescription>, )>;
type QuestEntryButtonQuery<'w, 's> = Query<'w, 's, (&'static Interaction, &'static QuestEntry, &'static mut BackgroundColor), (Changed<Interaction>, With<Button>, With<QuestEntrySlot>)>;

/// Game state that can block or change the journal, grouped to keep the system's
/// parameter list short (clippy::too_many_arguments).
#[derive(SystemParam)]
pub(crate) struct QuestUiContext<'w> {
    console: Res<'w, ChatConsole>,
    inventory: Res<'w, InventoryState>,
    sender: Res<'w, crate::net::NetworkSender>,
    combat: Option<Res<'w, CombatResource>>,
}

/// The three detail texts of the journal, grouped to keep the system's parameter list
/// short (clippy::too_many_arguments).
#[derive(SystemParam)]
pub(crate) struct QuestDetailTexts<'w, 's> {
    title_q: Query<'w, 's, &'static mut Text, With<QuestDetailTitle>>,
    desc_q: QuestDescriptionQuery<'w, 's>,
    obj_q: QuestObjectiveQuery<'w, 's>,
}

pub(crate) fn toggle_quest_ui(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<QuestState>,
    mut query: Query<&mut Visibility, With<QuestUiRoot>>,
    context: QuestUiContext,
    mut selected: ResMut<SelectedQuest>,
    texts: QuestDetailTexts,
) {
    let QuestDetailTexts {
        mut title_q,
        mut desc_q,
        mut obj_q,
    } = texts;
    let QuestUiContext {
        console,
        inventory,
        sender,
        combat,
    } = context;
    if let Some(ref combat) = combat {
        if combat.in_combat && state.open {
            state.open = false;
        }
        if combat.in_combat || combat.phase != crate::combat::CombatTransitionPhase::None {
            if state.is_changed() && let Ok(mut visibility) = query.get_single_mut() {
                *visibility = Visibility::Hidden;
            }
            return;
        }
    }

    if !console.open
        && !console.terminal_open
        && !inventory.open
        && keys.just_pressed(KeyCode::KeyU)
    {
        state.open = !state.open;
        if state.open {
            let _ = sender.0.send("QUESTS\n".to_string());
            // Reset detail panel to default "Select a quest" view.
            selected.id = None;
            if let Ok(mut t) = title_q.get_single_mut() {
                t.sections[0].value = "Select a quest".to_string();
            }
            if let Ok(mut t) = desc_q.get_single_mut() {
                t.sections[0].value = String::new();
            }
            if let Ok(mut t) = obj_q.get_single_mut() {
                t.sections[0].value = String::new();
            }
        }
    }

    if state.is_changed() && let Ok(mut visibility) = query.get_single_mut() {
        if state.open {
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

pub(crate) fn handle_quest_data(
    mut commands: Commands,
    mut events: EventReader<crate::net::ServerMessageEvent>,
    list_query: Query<Entity, With<QuestListPanel>>,
    existing_entries: Query<Entity, With<QuestEntrySlot>>,
    mut empty_text: Query<&mut Style, With<QuestEmptyText>>,
    mut selected: ResMut<SelectedQuest>,
    texts: QuestDetailTexts,
) {
    let QuestDetailTexts {
        mut title_q,
        mut desc_q,
        mut obj_q,
    } = texts;
    // Each event is a full list, so only the latest one of this frame matters.
    let Some(data) = events
        .read()
        .filter_map(|ev| ev.0.strip_prefix("S: EVT QUEST_DATA "))
        .last()
    else {
        return;
    };
    let Ok(list) = list_query.get_single() else {
        return;
    };

    // Remove old entries
    for entity in existing_entries.iter() {
        commands.entity(entity).despawn_recursive();
    }

    let data = data.trim();
    let entries: Vec<&str> = if data == "empty" {
        Vec::new()
    } else {
        data.split('|').collect()
    };
    if let Ok(mut style) = empty_text.get_single_mut() {
        style.display = if entries.is_empty() {
            Display::Flex
        } else {
            Display::None
        };
    }

    let mut selected_still_listed = false;
    for item in entries {
        // <id>:<name>:<description>:<objective>:<stage>:<counter>
        let parts: Vec<&str> = item.splitn(6, ':').collect();
        if parts.len() < 4 {
            continue;
        }
        let quest_id = parts[0].to_string();
        let quest_name = parts[1].to_string();
        let quest_desc = parts[2].to_string();
        let quest_obj = parts[3].to_string();
        let label = match parts.get(5).copied() {
            Some(counter) if !counter.is_empty() => format!("{}  ({})", quest_name, counter),
            _ => quest_name.clone(),
        };

        // Keep the open detail panel in sync with the new counters.
        let is_selected = selected.id.as_deref() == Some(quest_id.as_str());
        if is_selected {
            selected_still_listed = true;
            if let Ok(mut text) = desc_q.get_single_mut() {
                text.sections[0].value = quest_desc.clone();
            }
            if let Ok(mut text) = obj_q.get_single_mut() {
                text.sections[0].value = quest_obj.clone();
            }
        }

        let entry_entity = commands
            .spawn((
                ButtonBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(44.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        padding: UiRect::new(
                            Val::Px(12.0),
                            Val::Px(12.0),
                            Val::Px(8.0),
                            Val::Px(8.0),
                        ),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    background_color: if is_selected {
                        QUEST_ENTRY_SELECTED.into()
                    } else {
                        QUEST_ENTRY_BG.into()
                    },
                    border_color: QUEST_ENTRY_BORDER.into(),
                    ..default()
                },
                QuestEntrySlot,
                QuestEntry {
                    id: quest_id,
                    name: quest_name,
                    description: quest_desc,
                    objective: quest_obj,
                },
            ))
            .with_children(|btn| {
                btn.spawn(TextBundle::from_section(
                    label,
                    TextStyle {
                        font_size: 17.0,
                        color: QUEST_ENTRY_TEXT,
                        ..default()
                    },
                ));
            })
            .id();
        commands.entity(list).add_child(entry_entity);
    }

    // The selected quest just ended: back to the default detail panel.
    if selected.id.is_some() && !selected_still_listed {
        selected.id = None;
        if let Ok(mut text) = title_q.get_single_mut() {
            text.sections[0].value = "Select a quest".to_string();
        }
        if let Ok(mut text) = desc_q.get_single_mut() {
            text.sections[0].value = String::new();
        }
        if let Ok(mut text) = obj_q.get_single_mut() {
            text.sections[0].value = String::new();
        }
    }
}

pub(crate) fn handle_quest_selection(
    mut interaction_query: QuestEntryButtonQuery,
    mut title_q: Query<&mut Text, With<QuestDetailTitle>>,
    mut desc_q: QuestDescriptionQuery,
    mut obj_q: QuestObjectiveQuery,
    mut selected: ResMut<SelectedQuest>,
) {
    for (interaction, entry, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = QUEST_ENTRY_SELECTED.into();
                selected.id = Some(entry.id.clone());

                if let Ok(mut title) = title_q.get_single_mut() {
                    title.sections[0].value = entry.name.clone();
                }
                if let Ok(mut desc) = desc_q.get_single_mut() {
                    desc.sections[0].value = entry.description.clone();
                }
                if let Ok(mut obj) = obj_q.get_single_mut() {
                    obj.sections[0].value = entry.objective.clone();
                }
            }
            Interaction::Hovered => {
                if selected.id.as_deref() != Some(&entry.id) {
                    *color = QUEST_ENTRY_HOVER.into();
                }
            }
            Interaction::None => {
                if selected.id.as_deref() != Some(&entry.id) {
                    *color = QUEST_ENTRY_BG.into();
                }
            }
        }
    }
}
