use bevy::prelude::*;

use crate::net::{NetworkSender, ServerMessageEvent};
use crate::AppState;

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DialogueState>()
            .add_systems(Startup, setup_npc_dialogue_popup)
            .add_systems(
                Update,
                (
                    show_npc_dialogue_popup,
                    handle_dialogue_advance_input,
                    render_dialogue_choices,
                    handle_choice_click,
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[derive(Resource, Default)]
pub struct DialogueState {
    active: bool,
    lines: Vec<String>,
    current_index: usize,
    /// Current NPC (normalized id, used for ANSWER replies).
    pub current_npc: String,
    /// Available choices: (display text, answer value).
    pub choices: Vec<(String, String)>,
    /// True when showing the choice screen (after the last page).
    pub showing_choices: bool,
}

impl DialogueState {
    pub fn is_active(&self) -> bool {
        self.active
    }
}

// ── Composants UI ─────────────────────────────────────────────────────────

#[derive(Component)]
struct NpcDialoguePopupRoot;

#[derive(Component)]
struct NpcDialoguePortrait;

#[derive(Component)]
struct NpcDialogueText;

/// Container for choice buttons (shown instead of the text when
/// `showing_choices` is true).
#[derive(Component)]
struct DialogueChoicesRoot;

/// An individual choice button.
#[derive(Component)]
struct DialogueChoiceBtn {
    pub answer: String,
    pub npc: String,
}

fn setup_npc_dialogue_popup(mut commands: Commands) {
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                visibility: Visibility::Hidden,
                z_index: ZIndex::Global(9000),
                focus_policy: bevy::ui::FocusPolicy::Pass,
                ..default()
            },
            NpcDialoguePopupRoot,
        ))
        .with_children(|root| {
            root.spawn((
                ImageBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        left: Val::Px(24.0),
                        bottom: Val::Px(170.0),
                        width: Val::Px(220.0),
                        height: Val::Px(220.0),
                        ..default()
                    },
                    visibility: Visibility::Hidden,
                    ..default()
                },
                NpcDialoguePortrait,
            ));

            root.spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Px(170.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    ..default()
                },
                background_color: Color::rgba(0.06, 0.06, 0.08, 0.92).into(),
                ..default()
            })
            .with_children(|box_node| {
                box_node.spawn((
                    TextBundle::from_section(
                        "",
                        TextStyle {
                            font_size: 28.0,
                            color: Color::rgba(0.96, 0.96, 0.96, 1.0),
                            ..default()
                        },
                    )
                    .with_style(Style {
                        margin: UiRect {
                            left: Val::Px(240.0),
                            ..default()
                        },
                        ..default()
                    }),
                    NpcDialogueText,
                ));
            });
        });
}

fn show_npc_dialogue_popup(
    mut events: EventReader<ServerMessageEvent>,
    mobs: Query<(&crate::mob::Mob, &crate::player::PlayerName, &Handle<Image>)>,
    mut popup_q: Query<&mut Visibility, (With<NpcDialoguePopupRoot>, Without<NpcDialoguePortrait>)>,
    mut portrait_q: Query<(&mut Visibility, &mut UiImage), With<NpcDialoguePortrait>>,
    mut text_q: Query<&mut Text, With<NpcDialogueText>>,
    mut dialogue_state: ResMut<DialogueState>,
) {
    let Ok(mut popup_visibility) = popup_q.get_single_mut() else { return; };
    let Ok((mut portrait_visibility, mut portrait_image)) = portrait_q.get_single_mut() else { return; };
    let Ok(mut dialogue_text) = text_q.get_single_mut() else { return; };

    for ev in events.read() {
        // ── Choice dialogue ───────────────────────────────────────────────
        // Format: `S: EVT DIALOGUE_CHOICE <npc_key> <pages>||CHOICES||<text>:<ans>|…`
        if let Some(rest) = ev.0.strip_prefix("S: EVT DIALOGUE_CHOICE ") {
            let mut split = rest.splitn(2, ' ');
            let Some(speaker) = split.next() else { continue; };
            let Some(payload) = split.next() else { continue; };

            let (pages_part, choices_part) = if let Some(idx) = payload.find("||CHOICES||") {
                (&payload[..idx], &payload[idx + 11..])
            } else {
                (payload, "")
            };

            let normalized_speaker = speaker.replace('_', " ").trim().to_lowercase();
            let mut speaker_sprite: Option<Handle<Image>> = None;
            for (mob, name, sprite) in &mobs {
                if !mob.is_enemy && name.0.to_lowercase() == normalized_speaker {
                    speaker_sprite = Some(sprite.clone());
                    break;
                }
            }
            let lines: Vec<String> = pages_part
                .split('|')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect();
            let choices: Vec<(String, String)> = choices_part
                .split('|')
                .filter(|s| !s.is_empty())
                .filter_map(|c| {
                    let mut it = c.splitn(2, ':');
                    let text = it.next()?.replace('_', " ");
                    let ans  = it.next()?.to_string();
                    Some((text, ans))
                })
                .collect();

            if lines.is_empty() { continue; }
            if let Some(sprite) = speaker_sprite {
                portrait_image.texture = sprite;
                *portrait_visibility = Visibility::Inherited;
                *popup_visibility = Visibility::Inherited;
                dialogue_state.active = true;
                dialogue_state.current_index = 0;
                dialogue_state.lines = lines;
                dialogue_state.choices = choices;
                dialogue_state.showing_choices = false;
                dialogue_state.current_npc = speaker.to_string();
                dialogue_text.sections[0].value = dialogue_state.lines[0].clone();
            }
            continue;
        }

        // ── Dialogue simple ───────────────────────────────────────────────
        let Some(rest) = ev.0.strip_prefix("S: EVT DIALOGUE ") else { continue; };

        let mut split = rest.splitn(2, ' ');
        let Some(speaker) = split.next() else { continue; };
        let Some(dialogue_payload) = split.next() else { continue; };
        let normalized_speaker = speaker.replace('_', " ").trim().to_lowercase();

        let mut speaker_sprite: Option<Handle<Image>> = None;
        for (mob, name, sprite) in &mobs {
            if !mob.is_enemy && name.0.to_lowercase() == normalized_speaker {
                speaker_sprite = Some(sprite.clone());
                break;
            }
        }

        let lines: Vec<String> = dialogue_payload
            .split('|')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        if lines.is_empty() { continue; }

        if let Some(sprite) = speaker_sprite {
            portrait_image.texture = sprite;
            *portrait_visibility = Visibility::Inherited;
            *popup_visibility = Visibility::Inherited;
            dialogue_state.active = true;
            dialogue_state.current_index = 0;
            dialogue_state.lines = lines;
            dialogue_state.choices.clear();
            dialogue_state.showing_choices = false;
            dialogue_state.current_npc = speaker.to_string();
            dialogue_text.sections[0].value = dialogue_state.lines[0].clone();
        }
    }
}

fn handle_dialogue_advance_input(
    input: Res<ButtonInput<KeyCode>>,
    mut dialogue_state: ResMut<DialogueState>,
    mut popup_q: Query<&mut Visibility, (With<NpcDialoguePopupRoot>, Without<NpcDialoguePortrait>)>,
    mut portrait_q: Query<&mut Visibility, With<NpcDialoguePortrait>>,
    mut text_q: Query<&mut Text, With<NpcDialogueText>>,
    combat: Option<Res<crate::combat::CombatResource>>,
) {
    if let Some(ref combat) = combat {
        if combat.in_combat && dialogue_state.active {
            close_dialogue(&mut dialogue_state, &mut popup_q, &mut portrait_q, &mut text_q);
            return;
        }
    }

    // Do not advance while showing choices (handled by handle_choice_click).
    if dialogue_state.showing_choices { return; }
    if !dialogue_state.active || !input.just_pressed(KeyCode::KeyE) { return; }

    if dialogue_state.current_index + 1 < dialogue_state.lines.len() {
        dialogue_state.current_index += 1;
        if let Ok(mut dialogue_text) = text_q.get_single_mut() {
            dialogue_text.sections[0].value =
                dialogue_state.lines[dialogue_state.current_index].clone();
        }
        return;
    }

    // Last page — show choices if any, otherwise close.
    if !dialogue_state.choices.is_empty() {
        dialogue_state.showing_choices = true;
        if let Ok(mut dialogue_text) = text_q.get_single_mut() {
            dialogue_text.sections[0].value.clear();
        }
        return;
    }

    close_dialogue(&mut dialogue_state, &mut popup_q, &mut portrait_q, &mut text_q);
}

fn close_dialogue(
    dialogue_state: &mut DialogueState,
    popup_q: &mut Query<&mut Visibility, (With<NpcDialoguePopupRoot>, Without<NpcDialoguePortrait>)>,
    portrait_q: &mut Query<&mut Visibility, With<NpcDialoguePortrait>>,
    text_q: &mut Query<&mut Text, With<NpcDialogueText>>,
) {
    dialogue_state.active = false;
    dialogue_state.current_index = 0;
    dialogue_state.lines.clear();
    dialogue_state.choices.clear();
    dialogue_state.showing_choices = false;
    dialogue_state.current_npc.clear();
    if let Ok(mut v) = popup_q.get_single_mut()   { *v = Visibility::Hidden; }
    if let Ok(mut v) = portrait_q.get_single_mut() { *v = Visibility::Hidden; }
    if let Ok(mut t) = text_q.get_single_mut()     { t.sections[0].value.clear(); }
}

/// Spawns / despawns choice buttons based on `showing_choices`.
fn render_dialogue_choices(
    mut commands: Commands,
    dialogue_state: Res<DialogueState>,
    existing_root: Query<Entity, With<DialogueChoicesRoot>>,
) {
    if !dialogue_state.is_changed() { return; }

    // Clean up old buttons.
    for entity in &existing_root {
        commands.entity(entity).despawn_recursive();
    }

    if !dialogue_state.showing_choices || dialogue_state.choices.is_empty() {
        return;
    }

    // Create a vertical container of buttons.
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(260.0),
                    bottom: Val::Px(10.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                z_index: ZIndex::Global(9100),
                ..default()
            },
            DialogueChoicesRoot,
        ))
        .with_children(|parent| {
            for (text, answer) in &dialogue_state.choices {
                let npc = dialogue_state.current_npc.clone();
                parent
                    .spawn((
                        ButtonBundle {
                            style: Style {
                                padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                                ..default()
                            },
                            background_color: Color::rgba(0.15, 0.15, 0.2, 0.95).into(),
                            ..default()
                        },
                        DialogueChoiceBtn { answer: answer.clone(), npc },
                    ))
                    .with_children(|btn| {
                        btn.spawn(TextBundle::from_section(
                            text.clone(),
                            TextStyle {
                                font_size: 22.0,
                                color: Color::WHITE,
                                ..default()
                            },
                        ));
                    });
            }
        });
}

/// Handles a click on a choice button: sends `ANSWER <npc> <answer>` to the server
/// then closes the dialogue.
fn handle_choice_click(
    mut interaction_q: Query<
        (&Interaction, &DialogueChoiceBtn),
        (Changed<Interaction>, With<Button>),
    >,
    sender: Option<Res<NetworkSender>>,
    mut dialogue_state: ResMut<DialogueState>,
    mut popup_q: Query<&mut Visibility, (With<NpcDialoguePopupRoot>, Without<NpcDialoguePortrait>)>,
    mut portrait_q: Query<&mut Visibility, With<NpcDialoguePortrait>>,
    mut text_q: Query<&mut Text, With<NpcDialogueText>>,
) {
    let Some(sender) = sender else { return };

    for (interaction, btn) in &mut interaction_q {
        if *interaction == Interaction::Pressed {
            let msg = format!("ANSWER {} {}\n", btn.npc, btn.answer);
            let _ = sender.0.try_send(msg);
            close_dialogue(&mut dialogue_state, &mut popup_q, &mut portrait_q, &mut text_q);
        }
    }
}
