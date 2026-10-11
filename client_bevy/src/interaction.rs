use bevy::prelude::*;

use crate::net::ServerMessageEvent;
use crate::AppState;

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DialogueState>()
            .add_systems(Startup, setup_npc_dialogue_popup)
            .add_systems(
                Update,
                (show_npc_dialogue_popup, handle_dialogue_advance_input)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[derive(Component)]
struct NpcDialoguePopupRoot;

#[derive(Component)]
struct NpcDialoguePortrait;

#[derive(Component)]
struct NpcDialogueText;

#[derive(Resource, Default)]
pub struct DialogueState {
    active: bool,
    lines: Vec<String>,
    current_index: usize,
}

impl DialogueState {
    pub fn is_active(&self) -> bool {
        self.active
    }
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
    // Single-instance UI entities for the dialogue popup.
    let Ok(mut popup_visibility) = popup_q.get_single_mut() else {
        return;
    };
    let Ok((mut portrait_visibility, mut portrait_image)) = portrait_q.get_single_mut() else {
        return;
    };
    let Ok(mut dialogue_text) = text_q.get_single_mut() else {
        return;
    };

    for ev in events.read() {
        // Dedicated dialogue event sent by the server only for PNJ interaction.
        let Some(rest) = ev.0.strip_prefix("S: EVT DIALOGUE ") else {
            continue;
        };

        let mut split = rest.splitn(2, ' ');
        let Some(speaker) = split.next() else { continue; };
        let Some(dialogue_payload) = split.next() else { continue; };
        // Server uses underscores in names; normalize to match local display names.
        let normalized_speaker = speaker.replace('_', " ").trim().to_lowercase();

        let mut speaker_sprite: Option<Handle<Image>> = None;
        for (mob, name, sprite) in &mobs {
            // Only non-enemy NPCs can open this dialogue popup.
            if !mob.is_enemy && name.0.to_lowercase() == normalized_speaker {
                speaker_sprite = Some(sprite.clone());
                break;
            }
        }

        // Multiple dialogue pages are packed with "|" separators.
        let lines: Vec<String> = dialogue_payload
            .split('|')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        if lines.is_empty() {
            continue;
        }

        if let Some(sprite) = speaker_sprite {
            // Start a fresh dialogue session at page 1.
            portrait_image.texture = sprite;
            *portrait_visibility = Visibility::Inherited;
            *popup_visibility = Visibility::Inherited;
            dialogue_state.active = true;
            dialogue_state.current_index = 0;
            dialogue_state.lines = lines;
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
) {
    // E key is used only to advance/close while a dialogue is active.
    if !dialogue_state.active || !input.just_pressed(KeyCode::KeyE) {
        return;
    }

    // If there is another page, show it.
    if dialogue_state.current_index + 1 < dialogue_state.lines.len() {
        dialogue_state.current_index += 1;
        if let Ok(mut dialogue_text) = text_q.get_single_mut() {
            dialogue_text.sections[0].value =
                dialogue_state.lines[dialogue_state.current_index].clone();
        }
        return;
    }

    // Last page reached: close and clear dialogue state/UI.
    dialogue_state.active = false;
    dialogue_state.current_index = 0;
    dialogue_state.lines.clear();
    if let Ok(mut popup_visibility) = popup_q.get_single_mut() {
        *popup_visibility = Visibility::Hidden;
    }
    if let Ok(mut portrait_visibility) = portrait_q.get_single_mut() {
        *portrait_visibility = Visibility::Hidden;
    }
    if let Ok(mut dialogue_text) = text_q.get_single_mut() {
        dialogue_text.sections[0].value.clear();
    }
}
