use bevy::prelude::*;
use crate::AppState;
use crate::combat::CombatResource;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::LocalPlayerName;
use crate::ui::chat::components::{ChatInputContainer, ChatTabButton, TerminalInputText, TerminalLogText, TerminalUiRoot};
use crate::ui::chat::state::{ChatChannel, ChatConsole, ChatFadeState, ChatHistory};

pub(crate) fn toggle_chat(
    keys: Res<ButtonInput<KeyCode>>,
    mut console: ResMut<ChatConsole>,
    mut fade_state: ResMut<ChatFadeState>,
    combat: Option<Res<CombatResource>>,
) {
    if let Some(ref combat) = combat {
        if combat.in_combat || combat.phase != crate::combat::CombatTransitionPhase::None {
            return;
        }
    }
    if console.terminal_open {
        return;
    }
    if !console.open {
        if keys.just_pressed(KeyCode::KeyT) || keys.just_pressed(KeyCode::Enter) {
            console.open = true;
            console.just_opened = true;
            fade_state.reset();
        } else if keys.just_pressed(KeyCode::Slash) {
            console.open = true;
            console.just_opened = true;
            console.input_buffer = "/".to_string();
            fade_state.reset();
        }
    } else {
        if keys.just_pressed(KeyCode::Escape) {
            console.open = false;
            console.input_buffer.clear();
        } else if keys.just_pressed(KeyCode::Tab) {
            console.cycle_channel();
            fade_state.reset();
        }
    }
}

pub(crate) fn toggle_terminal(
    keys: Res<ButtonInput<KeyCode>>,
    mut console: ResMut<ChatConsole>,
    mut query: Query<&mut Visibility, With<TerminalUiRoot>>,
    combat: Option<Res<CombatResource>>,
) {
    if let Some(ref combat) = combat {
        if combat.in_combat || combat.phase != crate::combat::CombatTransitionPhase::None {
            return;
        }
    }
    let toggle_pressed = keys.just_pressed(KeyCode::F1)
        || keys.just_pressed(KeyCode::Backquote)
        || keys.just_pressed(KeyCode::F2);

    if toggle_pressed {
        console.terminal_open = !console.terminal_open;
        console.terminal_just_opened = console.terminal_open;
        if console.terminal_open {
            console.open = false;
        }
        if let Ok(mut vis) = query.get_single_mut() {
            *vis = if console.terminal_open {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    } else if console.terminal_open && keys.just_pressed(KeyCode::Escape) {
        console.terminal_open = false;
        if let Ok(mut vis) = query.get_single_mut() {
            *vis = Visibility::Hidden;
        }
    }
}

pub(crate) fn handle_tab_clicks(
    interaction_q: Query<(&Interaction, &ChatTabButton), (Changed<Interaction>, With<Button>)>,
    mut console: ResMut<ChatConsole>,
    mut fade_state: ResMut<ChatFadeState>,
    combat: Option<Res<CombatResource>>,
) {
    if let Some(ref combat) = combat {
        if combat.in_combat || combat.phase != crate::combat::CombatTransitionPhase::None {
            return;
        }
    }
    for (interaction, btn) in &interaction_q {
        if *interaction == Interaction::Pressed {
            console.active_channel = btn.0;
            console.clear_unread_for_active();
            fade_state.reset();
        }
    }
}

pub(crate) fn handle_input_clicks(
    interaction_q: Query<&Interaction, (Changed<Interaction>, With<ChatInputContainer>)>,
    mut console: ResMut<ChatConsole>,
    mut fade_state: ResMut<ChatFadeState>,
    combat: Option<Res<CombatResource>>,
) {
    if let Some(ref combat) = combat {
        if combat.in_combat || combat.phase != crate::combat::CombatTransitionPhase::None {
            return;
        }
    }
    for interaction in &interaction_q {
        if *interaction == Interaction::Pressed {
            console.open = true;
            fade_state.reset();
        }
    }
}

pub(crate) fn handle_terminal_inputs(
    mut char_evr: EventReader<ReceivedCharacter>,
    keys: Res<ButtonInput<KeyCode>>,
    sender: Res<NetworkSender>,
    mut console: ResMut<ChatConsole>,
    mut input_query: Query<&mut Text, (With<TerminalInputText>, Without<TerminalLogText>)>,
    mut log_query: Query<&mut Text, (With<TerminalLogText>, Without<TerminalInputText>)>,
    combat: Option<Res<CombatResource>>,
) {
    if let Some(ref combat) = combat {
        if combat.in_combat || combat.phase != crate::combat::CombatTransitionPhase::None {
            return;
        }
    }
    if !console.terminal_open {
        return;
    }
    let swallow_chars = std::mem::take(&mut console.terminal_just_opened);

    let Ok(mut input_text) = input_query.get_single_mut() else {
        return;
    };

    for ev in char_evr.read() {
        if swallow_chars {
            continue;
        }
        let s = ev.char.to_string();
        if !s.contains('\u{8}')
            && !s.contains('\r')
            && !s.contains('\n')
            && !s.contains('`')
            && !s.contains('~')
            && !s.contains('²')
        {
            input_text.sections[0].value.push_str(&s);
        }
    }

    if keys.just_pressed(KeyCode::Backspace) && input_text.sections[0].value.chars().count() > 2 {
        input_text.sections[0].value.pop();
    }

    if keys.just_pressed(KeyCode::Enter) {
        let command = input_text.sections[0].value[2..].trim().to_string();
        if !command.is_empty() {
            let _ = sender.0.send(format!("{}\n", command));
            if let Ok(mut log_text) = log_query.get_single_mut() {
                log_text.sections[0]
                    .value
                    .push_str(&format!("> {}\n", command));
                let lines: Vec<&str> = log_text.sections[0].value.lines().collect();
                if lines.len() > 15 {
                    log_text.sections[0].value = lines[lines.len() - 15..].join("\n") + "\n";
                }
            }
            input_text.sections[0].value = "> ".to_string();
        }
    }
}

pub(crate) fn display_terminal_messages(
    mut events: EventReader<ServerMessageEvent>,
    mut query: Query<&mut Text, With<TerminalLogText>>,
) {
    for ev in events.read() {
        for mut text in query.iter_mut() {
            text.sections[0]
                .value
                .push_str(&format!("{}\n", ev.0.trim()));
            let lines: Vec<&str> = text.sections[0].value.lines().collect();
            if lines.len() > 15 {
                text.sections[0].value = lines[lines.len() - 15..].join("\n") + "\n";
            }
        }
    }
}

pub(crate) fn handle_inputs(
    mut char_evr: EventReader<ReceivedCharacter>,
    keys: Res<ButtonInput<KeyCode>>,
    sender: Res<NetworkSender>,
    mut console: ResMut<ChatConsole>,
    mut history: ResMut<ChatHistory>,
    local_name: Res<LocalPlayerName>,
    combat: Option<Res<CombatResource>>,
) {
    if let Some(ref combat) = combat {
        if combat.in_combat || combat.phase != crate::combat::CombatTransitionPhase::None {
            return;
        }
    }
    if !console.open || console.terminal_open {
        return;
    }
    let swallow_chars = std::mem::take(&mut console.just_opened);

    for ev in char_evr.read() {
        if swallow_chars {
            continue;
        }
        let s = ev.char.to_string();
        if !s.contains('\u{8}') && !s.contains('\r') && !s.contains('\n') && !s.contains('\t') {
            console.input_buffer.push_str(&s);
        }
    }

    if keys.just_pressed(KeyCode::Backspace) {
        console.input_buffer.pop();
    }

    if keys.just_pressed(KeyCode::Enter) {
        let msg = console.input_buffer.trim().to_string();
        if !msg.is_empty() {
            let my_name = local_name.0.clone().unwrap_or_else(|| "Moi".to_string());
            if msg.starts_with('/') {
                let cmd = msg[1..].trim();
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                let first = parts.first().map(|s| s.to_uppercase()).unwrap_or_default();

                match first.as_str() {
                    "HELP" => {
                        history.add(
                            console.active_channel,
                            "Commandes: /group (create|invite <nom>|accept|leave), /who, /global <msg>, /room <msg>, /group <msg>".to_string(),
                            Color::rgb(0.95, 0.85, 0.40),
                        );
                    }
                    "INVITE" if parts.len() > 1 => {
                        let _ = sender
                            .0
                            .send(format!("GROUP INVITE {}\n", parts[1..].join(" ")));
                    }
                    "ACCEPT" => {
                        let _ = sender.0.send("GROUP ACCEPT\n".to_string());
                    }
                    "LEAVE" => {
                        let _ = sender.0.send("GROUP LEAVE\n".to_string());
                    }
                    "GLOBAL" if parts.len() > 1 => {
                        let text = parts[1..].join(" ");
                        let _ = sender.0.send(format!("CHAT GLOBAL {}\n", text));
                        history.add(
                            ChatChannel::Global,
                            format!("[Global] {}: {}", my_name, text),
                            Color::rgb(0.95, 0.90, 0.80),
                        );
                        console.last_sent =
                            Some((ChatChannel::Global, text, std::time::Instant::now()));
                    }
                    "ROOM" if parts.len() > 1 => {
                        let text = parts[1..].join(" ");
                        let _ = sender.0.send(format!("CHAT ROOM {}\n", text));
                        history.add(
                            ChatChannel::Room,
                            format!("[Room] {}: {}", my_name, text),
                            Color::rgb(0.85, 1.0, 0.88),
                        );
                        console.last_sent =
                            Some((ChatChannel::Room, text, std::time::Instant::now()));
                    }
                    "GROUP"
                        if parts.len() > 1
                            && !["CREATE", "INVITE", "ACCEPT", "LEAVE", "INFO"]
                                .contains(&parts[1].to_uppercase().as_str()) =>
                    {
                        let text = parts[1..].join(" ");
                        let _ = sender.0.send(format!("CHAT GROUP {}\n", text));
                        history.add(
                            ChatChannel::Group,
                            format!("[Group] {}: {}", my_name, text),
                            Color::rgb(0.80, 0.90, 1.0),
                        );
                        console.last_sent =
                            Some((ChatChannel::Group, text, std::time::Instant::now()));
                    }
                    _ => {
                        let _ = sender.0.send(format!("{}\n", cmd));
                    }
                }
            } else if msg.to_uppercase().starts_with("CHAT ") {
                let _ = sender.0.send(format!("{}\n", msg));
            } else {
                let channel_cmd = match console.active_channel {
                    ChatChannel::Global => "GLOBAL",
                    ChatChannel::Room => "ROOM",
                    ChatChannel::Group => "GROUP",
                };
                let _ = sender.0.send(format!("CHAT {} {}\n", channel_cmd, msg));

                // Immediate insertion for guaranteed instant display
                let (prefix, color) = match console.active_channel {
                    ChatChannel::Global => (
                        format!("[Global] {}: {}", my_name, msg),
                        Color::rgb(0.95, 0.90, 0.80),
                    ),
                    ChatChannel::Room => (
                        format!("[Room] {}: {}", my_name, msg),
                        Color::rgb(0.85, 1.0, 0.88),
                    ),
                    ChatChannel::Group => (
                        format!("[Group] {}: {}", my_name, msg),
                        Color::rgb(0.80, 0.90, 1.0),
                    ),
                };
                history.add(console.active_channel, prefix, color);
                console.last_sent = Some((
                    console.active_channel,
                    msg.clone(),
                    std::time::Instant::now(),
                ));
            }
            console.input_buffer.clear();
        }
        console.open = false;
    }
}
