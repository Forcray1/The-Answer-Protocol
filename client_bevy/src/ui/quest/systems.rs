use bevy::prelude::*;
use crate::AppState;
use crate::combat::CombatResource;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::LocalPlayerName;
use crate::ui::chat::ChatConsole;
use crate::ui::inventory::InventoryState;
use crate::ui::quest::components::{QUEST_ENTRY_BG, QUEST_ENTRY_BORDER, QUEST_ENTRY_HOVER, QUEST_ENTRY_SELECTED, QUEST_ENTRY_TEXT, QuestDetailDescription, QuestDetailObjective, QuestDetailTitle, QuestEmptyText, QuestEntry, QuestEntrySlot, QuestListPanel, QuestState, QuestUiRoot, SelectedQuest};

pub(crate) fn toggle_quest_ui(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<QuestState>,
    mut query: Query<&mut Visibility, With<QuestUiRoot>>,
    console: Res<ChatConsole>,
    inventory: Res<InventoryState>,
    sender: Res<crate::net::NetworkSender>,
    combat: Option<Res<CombatResource>>,
    mut selected: ResMut<SelectedQuest>,
    mut title_q: Query<&mut Text, With<QuestDetailTitle>>,
    mut desc_q: Query<
        &mut Text,
        (
            With<QuestDetailDescription>,
            Without<QuestDetailTitle>,
            Without<QuestDetailObjective>,
        ),
    >,
    mut obj_q: Query<
        &mut Text,
        (
            With<QuestDetailObjective>,
            Without<QuestDetailTitle>,
            Without<QuestDetailDescription>,
        ),
    >,
) {
    if let Some(ref combat) = combat {
        if combat.in_combat && state.open {
            state.open = false;
        }
        if combat.in_combat || combat.phase != crate::combat::CombatTransitionPhase::None {
            if state.is_changed() {
                if let Ok(mut visibility) = query.get_single_mut() {
                    *visibility = Visibility::Hidden;
                }
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

    if state.is_changed() {
        if let Ok(mut visibility) = query.get_single_mut() {
            if state.open {
                *visibility = Visibility::Inherited;
            } else {
                *visibility = Visibility::Hidden;
            }
        }
    }
}

pub(crate) fn handle_quest_data(
    mut commands: Commands,
    mut events: EventReader<crate::net::ServerMessageEvent>,
    list_query: Query<Entity, With<QuestListPanel>>,
    existing_entries: Query<Entity, With<QuestEntrySlot>>,
    mut empty_text: Query<&mut Style, With<QuestEmptyText>>,
) {
    for ev in events.read() {
        if let Some(data) = ev.0.strip_prefix("S: EVT QUEST_DATA ") {
            let Ok(list) = list_query.get_single() else {
                continue;
            };

            // Remove old entries
            for entity in existing_entries.iter() {
                commands.entity(entity).despawn_recursive();
            }

            if data.trim() == "empty" {
                // Show "No active quests"
                if let Ok(mut style) = empty_text.get_single_mut() {
                    style.display = Display::Flex;
                }
                continue;
            }

            // Hide empty text
            if let Ok(mut style) = empty_text.get_single_mut() {
                style.display = Display::None;
            }

            for item in data.trim().split('|') {
                let parts: Vec<&str> = item.splitn(5, ':').collect();
                if parts.len() >= 4 {
                    let quest_id = parts[0].to_string();
                    let quest_name = parts[1].to_string();
                    let quest_desc = parts[2].to_string();
                    // parts[3] = objective text, parts[4] = stage wire (drop it from display)
                    let quest_obj = parts[3].to_string();

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
                                background_color: QUEST_ENTRY_BG.into(),
                                border_color: QUEST_ENTRY_BORDER.into(),
                                ..default()
                            },
                            QuestEntrySlot,
                            QuestEntry {
                                id: quest_id,
                                name: quest_name.clone(),
                                description: quest_desc,
                                objective: quest_obj,
                            },
                        ))
                        .with_children(|btn| {
                            btn.spawn(TextBundle::from_section(
                                quest_name,
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
            }
        }
    }
}

pub(crate) fn handle_quest_selection(
    mut interaction_query: Query<
        (&Interaction, &QuestEntry, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>, With<QuestEntrySlot>),
    >,
    mut title_q: Query<&mut Text, With<QuestDetailTitle>>,
    mut desc_q: Query<
        &mut Text,
        (
            With<QuestDetailDescription>,
            Without<QuestDetailTitle>,
            Without<QuestDetailObjective>,
        ),
    >,
    mut obj_q: Query<
        &mut Text,
        (
            With<QuestDetailObjective>,
            Without<QuestDetailTitle>,
            Without<QuestDetailDescription>,
        ),
    >,
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
