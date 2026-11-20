use bevy::prelude::*;
use crate::AppState;
use crate::combat::CombatResource;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::LocalPlayerName;
use crate::ui::chat::components::{ChatInputContainer, ChatInputPrompt, ChatInputText, ChatMessagesArea, ChatShortcutHelpText, ChatTabButton, ChatTabText, ChatText, ChatUiRoot, TerminalUiRoot};
use crate::ui::chat::MAX_CHAT_LINES;
use crate::ui::chat::state::{CHAT_FADE_DURATION, CHAT_INACTIVITY_TIMEOUT, ChatChannel, ChatConsole, ChatFadeState, ChatHistory, ChatMessage};

pub(crate) fn tick_chat_fade(
    time: Res<Time>,
    console: Res<ChatConsole>,
    history: Res<ChatHistory>,
    mut fade_state: ResMut<ChatFadeState>,
    mut chat_root_q: Query<(&mut Visibility, Option<&Interaction>), With<ChatUiRoot>>,
    combat: Option<Res<CombatResource>>,
) {
    if combat.as_ref().map_or(false, |c| c.in_combat) {
        return;
    }

    // New messages in history wake up the chat and reset the inactivity timer
    if history.is_changed() {
        fade_state.reset();
    }

    // Hovering over the visible chatbox keeps it alive
    if fade_state.alpha > 0.0 {
        if let Ok((_, Some(interaction))) = chat_root_q.get_single() {
            if *interaction == Interaction::Hovered {
                fade_state.bypass_change_detection().activity_timer = CHAT_INACTIVITY_TIMEOUT;
            }
        }
    }

    // While typing, stay completely visible
    if console.open {
        if fade_state.alpha != 1.0 {
            fade_state.alpha = 1.0;
        }
        fade_state.bypass_change_detection().activity_timer = CHAT_INACTIVITY_TIMEOUT;
    } else {
        let dt = time.delta_seconds();
        if fade_state.activity_timer > 0.0 {
            fade_state.bypass_change_detection().activity_timer =
                (fade_state.activity_timer - dt).max(0.0);
        } else if fade_state.alpha > 0.0 {
            let new_alpha = (fade_state.alpha - dt / CHAT_FADE_DURATION).max(0.0);
            fade_state.alpha = new_alpha;
        }
    }

    // Manage visibility of the root container
    if let Ok((mut vis, _)) = chat_root_q.get_single_mut() {
        if fade_state.alpha <= 0.0 {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        } else if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
    }
}

pub(crate) fn update_chat_combat_visibility(
    combat: Option<Res<CombatResource>>,
    mut chat_root_q: Query<&mut Visibility, (With<ChatUiRoot>, Without<TerminalUiRoot>)>,
    mut term_root_q: Query<&mut Visibility, (With<TerminalUiRoot>, Without<ChatUiRoot>)>,
    mut console: ResMut<ChatConsole>,
    mut fade_state: ResMut<ChatFadeState>,
) {
    let Some(combat) = combat else { return };
    if combat.is_changed() {
        if combat.in_combat {
            if let Ok(mut vis) = chat_root_q.get_single_mut() {
                *vis = Visibility::Hidden;
            }
            if let Ok(mut vis) = term_root_q.get_single_mut() {
                *vis = Visibility::Hidden;
            }
            console.open = false;
            console.terminal_open = false;
            console.input_buffer.clear();
            fade_state.alpha = 0.0;
            fade_state.activity_timer = 0.0;
        } else {
            fade_state.reset();
            if let Ok(mut vis) = chat_root_q.get_single_mut() {
                *vis = Visibility::Inherited;
            }
        }
    }
}

pub(crate) fn update_chat_container_style(
    console: Res<ChatConsole>,
    fade_state: Res<ChatFadeState>,
    mut query: Query<(&mut BackgroundColor, &mut BorderColor), With<ChatUiRoot>>,
) {
    if !console.is_changed() && !fade_state.is_changed() {
        return;
    }
    let alpha = fade_state.alpha;
    if let Ok((mut bg, mut border)) = query.get_single_mut() {
        if console.open {
            *bg = Color::rgba(0.06, 0.07, 0.10, 0.92 * alpha).into();
            *border = Color::rgba(0.45, 0.45, 0.55, 0.85 * alpha).into();
        } else {
            *bg = Color::rgba(0.04, 0.05, 0.07, 0.65 * alpha).into();
            *border = Color::rgba(0.25, 0.25, 0.35, 0.45 * alpha).into();
        }
    }
}

pub(crate) fn update_chat_tabs_ui(
    console: Res<ChatConsole>,
    fade_state: Res<ChatFadeState>,
    mut tab_buttons: Query<(
        &ChatTabButton,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut Style,
    )>,
    mut tab_texts: Query<(&ChatTabText, &mut Text), Without<ChatShortcutHelpText>>,
    mut help_texts: Query<&mut Text, With<ChatShortcutHelpText>>,
) {
    if !console.is_changed() && !fade_state.is_changed() {
        return;
    }
    let alpha = fade_state.alpha;
    for (btn, mut bg, mut border, mut style) in &mut tab_buttons {
        if btn.0 == ChatChannel::Group {
            style.display = if console.in_group {
                Display::Flex
            } else {
                Display::None
            };
        }

        if btn.0 == console.active_channel {
            *bg = Color::rgba(0.20, 0.22, 0.30, 0.95 * alpha).into();
            *border = btn.0.color().with_a(alpha).into();
        } else {
            *bg = Color::rgba(0.08, 0.08, 0.12, 0.60 * alpha).into();
            *border = Color::rgba(0.20, 0.20, 0.28, 0.40 * alpha).into();
        }
    }

    for (tab_text, mut text) in &mut tab_texts {
        let is_active = tab_text.0 == console.active_channel;
        let unread = match tab_text.0 {
            ChatChannel::Global => console.unread_global,
            ChatChannel::Room => console.unread_room,
            ChatChannel::Group => console.unread_group,
        };

        if is_active {
            text.sections[0].value = tab_text.0.name().to_string();
            text.sections[0].style.color = tab_text.0.color().with_a(alpha);
        } else if unread > 0 {
            text.sections[0].value = format!("{} (*)", tab_text.0.name());
            text.sections[0].style.color = tab_text.0.color().with_a(alpha);
        } else {
            text.sections[0].value = tab_text.0.name().to_string();
            text.sections[0].style.color = Color::rgba(0.60, 0.60, 0.65, 0.75 * alpha);
        }
    }

    for mut text in &mut help_texts {
        text.sections[0].style.color = Color::rgba(0.55, 0.55, 0.62, 0.70 * alpha);
    }
}

pub(crate) fn update_chat_input_ui(
    time: Res<Time>,
    mut console: ResMut<ChatConsole>,
    fade_state: Res<ChatFadeState>,
    mut prompt_q: Query<&mut Text, (With<ChatInputPrompt>, Without<ChatInputText>)>,
    mut input_text_q: Query<&mut Text, (With<ChatInputText>, Without<ChatInputPrompt>)>,
    mut container_q: Query<(&mut BackgroundColor, &mut BorderColor), With<ChatInputContainer>>,
) {
    if console.open {
        console.cursor_timer.tick(time.delta());
        if console.cursor_timer.just_finished() {
            console.cursor_visible = !console.cursor_visible;
        }
    }

    let alpha = fade_state.alpha;
    let channel_color = console.active_channel.color();

    if let Ok(mut prompt) = prompt_q.get_single_mut() {
        prompt.sections[0].value = if console.open {
            format!("[{}] > ", console.active_channel.name())
        } else {
            format!("[{}] ", console.active_channel.name())
        };
        prompt.sections[0].style.color = if console.open {
            channel_color.with_a(alpha)
        } else {
            channel_color.with_a(0.65 * alpha)
        };
    }

    if let Ok(mut text) = input_text_q.get_single_mut() {
        if console.open {
            let cursor = if console.cursor_visible { "_" } else { " " };
            text.sections[0].value = format!("{}{}", console.input_buffer, cursor);
            text.sections[0].style.color = Color::rgba(1.0, 1.0, 1.0, alpha);
            text.sections[0].style.font_size = 14.0;
        } else {
            text.sections[0].value = "Press 'T' to chat...".to_string();
            text.sections[0].style.color = Color::rgba(0.55, 0.55, 0.60, 0.70 * alpha);
            text.sections[0].style.font_size = 13.0;
        }
    }

    if let Ok((mut bg, mut border)) = container_q.get_single_mut() {
        if console.open {
            *bg = Color::rgba(0.08, 0.09, 0.13, 0.95 * alpha).into();
            *border = channel_color.with_a(alpha).into();
        } else {
            *bg = Color::rgba(0.04, 0.05, 0.08, 0.70 * alpha).into();
            *border = Color::rgba(0.25, 0.25, 0.35, 0.50 * alpha).into();
        }
    }
}

pub(crate) fn update_chat_messages_ui(
    console: Res<ChatConsole>,
    fade_state: Res<ChatFadeState>,
    history: Res<ChatHistory>,
    mut query: Query<&mut Text, With<ChatText>>,
    mut msg_area_q: Query<&mut BackgroundColor, With<ChatMessagesArea>>,
) {
    if !history.is_changed() && !console.is_changed() && !fade_state.is_changed() {
        return;
    }

    let alpha = fade_state.alpha;

    for mut bg in &mut msg_area_q {
        *bg = Color::rgba(0.02, 0.03, 0.05, 0.50 * alpha).into();
    }

    let Ok(mut text) = query.get_single_mut() else {
        return;
    };

    let relevant: Vec<&ChatMessage> = history
        .messages
        .iter()
        .filter(|m| m.channel == console.active_channel)
        .rev()
        .take(MAX_CHAT_LINES)
        .collect();

    let mut sections = Vec::new();
    if relevant.is_empty() {
        sections.push(TextSection::new(
            format!(
                "(No messages in channel {})\n",
                console.active_channel.name()
            ),
            TextStyle {
                font_size: 14.0,
                color: Color::rgba(0.5, 0.5, 0.55, 0.70 * alpha),
                ..default()
            },
        ));
    } else {
        for msg in relevant.into_iter().rev() {
            sections.push(TextSection::new(
                format!("{}\n", msg.text),
                TextStyle {
                    font_size: 14.0,
                    color: msg.color.with_a(alpha),
                    ..default()
                },
            ));
        }
    }
    text.sections = sections;
}
