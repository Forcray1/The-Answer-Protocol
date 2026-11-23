use bevy::prelude::*;
use crate::net::ServerMessageEvent;
use crate::player::LocalPlayerName;
use crate::ui::chat::state::{ChatChannel, ChatConsole, ChatHistory};

pub(crate) fn process_chat_events(
    mut events: EventReader<ServerMessageEvent>,
    mut console: ResMut<ChatConsole>,
    mut history: ResMut<ChatHistory>,
    local_name: Res<LocalPlayerName>,
) {
    for ev in events.read() {
        let line = ev.0.trim();

        // GLOBAL CHAT: S: EVT GLOBAL CHAT <sender> <message>
        if let Some(rest) = line.strip_prefix("S: EVT GLOBAL CHAT ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 2 {
                let sender = parts[0];
                let message = parts[1..].join(" ");
                let is_recent_self = local_name.0.as_deref() == Some(sender)
                    && console.last_sent.as_ref().is_some_and(|(c, m, t)| {
                        *c == ChatChannel::Global && m == &message && t.elapsed().as_secs() < 3
                    });

                if !is_recent_self {
                    if sender == "Server" {
                        history.add(
                            ChatChannel::Global,
                            format!("[Server] {}", message),
                            Color::rgb(1.0, 0.85, 0.3),
                        );
                    } else {
                        history.add(
                            ChatChannel::Global,
                            format!("[Global] {}: {}", sender, message),
                            Color::rgb(0.95, 0.90, 0.80),
                        );
                    }
                    if console.active_channel != ChatChannel::Global {
                        console.unread_global += 1;
                    }
                }
            }
            continue;
        }

        // ROOM EVENTS: S: EVT ROOM <verb> ...
        if line.starts_with("S: EVT ROOM ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                match parts[3] {
                    "CHAT" if parts.len() >= 6 => {
                        let sender = parts[4].replace('_', " ");
                        let message = parts[5..].join(" ");
                        let is_recent_self = local_name.0.as_deref() == Some(sender.as_str())
                            && console.last_sent.as_ref().is_some_and(|(c, m, t)| {
                                *c == ChatChannel::Room
                                    && m == &message
                                    && t.elapsed().as_secs() < 3
                            });

                        if !is_recent_self {
                            let display = if message.starts_with('[') && message.contains(" has dropped ") {
                                format!("[Room] {}", message)
                            } else {
                                format!("[Room] {}: {}", sender, message)
                            };
                            history.add(ChatChannel::Room, display, Color::rgb(0.85, 1.0, 0.88));
                            if console.active_channel != ChatChannel::Room {
                                console.unread_room += 1;
                            }
                        }
                    }
                    "PRESENCE" if parts.len() >= 6 => {
                        let action = parts[4];
                        let user = parts[5];
                        let is_me = local_name.0.as_deref() == Some(user);
                        if !is_me {
                            if action == "LEAVE" {
                                history.add(
                                    ChatChannel::Room,
                                    format!("[Room] {} left the room", user),
                                    Color::rgb(0.70, 0.70, 0.75),
                                );
                                if console.active_channel != ChatChannel::Room {
                                    console.unread_room += 1;
                                }
                            } else if action == "ENTER" {
                                let is_room_loading = console
                                    .room_change_time
                                    .as_ref()
                                    .is_some_and(|t| t.elapsed().as_millis() < 800);
                                if !is_room_loading {
                                    history.add(
                                        ChatChannel::Room,
                                        format!("[Room] {} entered the room", user),
                                        Color::rgb(0.70, 0.70, 0.75),
                                    );
                                    if console.active_channel != ChatChannel::Room {
                                        console.unread_room += 1;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            continue;
        }

        // ROOM LOCATION CHANGE: S: OK room=loc.<room>
        if let Some(room) = line.strip_prefix("S: OK room=loc.") {
            console.room_change_time = Some(std::time::Instant::now());
            history.add(
                ChatChannel::Room,
                format!("[Room] Arrived in: {}", room),
                Color::rgb(0.40, 0.80, 0.90),
            );
            continue;
        }

        if line.starts_with("S: OK connected") {
            console.room_change_time = Some(std::time::Instant::now());
        }

        // GROUP EVENTS: S: EVT GROUP ...
        if line.starts_with("S: EVT GROUP ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                match parts[3] {
                    "CHAT" if parts.len() >= 6 => {
                        let sender = parts[4];
                        let message = parts[5..].join(" ");
                        let is_recent_self = local_name.0.as_deref() == Some(sender)
                            && console.last_sent.as_ref().is_some_and(|(c, m, t)| {
                                *c == ChatChannel::Group
                                    && m == &message
                                    && t.elapsed().as_secs() < 3
                            });

                        if !is_recent_self {
                            history.add(
                                ChatChannel::Group,
                                format!("[Group] {}: {}", sender, message),
                                Color::rgb(0.80, 0.90, 1.0),
                            );
                            if console.active_channel != ChatChannel::Group {
                                console.unread_group += 1;
                            }
                        }
                    }
                    "JOIN" if parts.len() >= 5 => {
                        let user = parts[4];
                        history.add(
                            ChatChannel::Group,
                            format!("[Group] {} joined the group", user),
                            Color::rgb(0.50, 0.85, 1.0),
                        );
                        if local_name.0.as_deref() == Some(user) {
                            console.in_group = true;
                        }
                    }
                    "LEAVE" if parts.len() >= 5 => {
                        let user = parts[4];
                        history.add(
                            ChatChannel::Group,
                            format!("[Group] {} left the group", user),
                            Color::rgb(0.70, 0.75, 0.90),
                        );
                        if local_name.0.as_deref() == Some(user) {
                            console.in_group = false;
                            if console.active_channel == ChatChannel::Group {
                                console.active_channel = ChatChannel::Room;
                            }
                        }
                    }
                    "CREATED" if parts.len() >= 5 => {
                        let user = parts[4];
                        history.add(
                            ChatChannel::Group,
                            format!("[Group] {} created the group", user),
                            Color::rgb(0.50, 0.85, 1.0),
                        );
                        if local_name.0.as_deref() == Some(user) {
                            console.in_group = true;
                        }
                    }
                    "DISBAND" => {
                        let text = parts[4..].join(" ");
                        history.add(
                            ChatChannel::Group,
                            format!("[Group] {}", text),
                            Color::rgb(1.0, 0.6, 0.4),
                        );
                        history.add(
                            ChatChannel::Room,
                            format!("[Group] {}", text),
                            Color::rgb(1.0, 0.6, 0.4),
                        );
                        console.in_group = false;
                        if console.active_channel == ChatChannel::Group {
                            console.active_channel = ChatChannel::Room;
                        }
                    }
                    "INVITED" => {
                        let text = parts[4..].join(" ");
                        let msg = format!("[Group] {}", text);
                        history.add(ChatChannel::Room, msg.clone(), Color::rgb(1.0, 0.85, 0.4));
                        history.add(ChatChannel::Global, msg, Color::rgb(1.0, 0.85, 0.4));
                    }
                    _ => {}
                }
            }
            continue;
        }

        // SERVER RESPONSES
        if line == "S: OK you_joined_the_group" {
            console.in_group = true;
            history.add(
                ChatChannel::Room,
                "[Group] You joined the group!".to_string(),
                Color::rgb(0.5, 0.85, 1.0),
            );
            history.add(
                ChatChannel::Group,
                "[Group] You joined the group!".to_string(),
                Color::rgb(0.5, 0.85, 1.0),
            );
        } else if line == "S: OK group_created" {
            console.in_group = true;
            history.add(
                ChatChannel::Room,
                "[Group] Group created!".to_string(),
                Color::rgb(0.5, 0.85, 1.0),
            );
            history.add(
                ChatChannel::Group,
                "[Group] Group created!".to_string(),
                Color::rgb(0.5, 0.85, 1.0),
            );
        } else if line == "S: OK you_left_the_group" {
            console.in_group = false;
            if console.active_channel == ChatChannel::Group {
                console.active_channel = ChatChannel::Room;
            }
            history.add(
                ChatChannel::Room,
                "[Group] You left the group.".to_string(),
                Color::rgb(0.7, 0.7, 0.8),
            );
        } else if line == "S: OK group_disbanded" {
            console.in_group = false;
            if console.active_channel == ChatChannel::Group {
                console.active_channel = ChatChannel::Room;
            }
            history.add(
                ChatChannel::Room,
                "[Group] The group was disbanded.".to_string(),
                Color::rgb(1.0, 0.6, 0.4),
            );
        } else if let Some(members) = line.strip_prefix("S: OK Group members: ") {
            console.in_group = true;
            history.add(
                ChatChannel::Group,
                format!("[Group] Members: {}", members),
                Color::rgb(0.5, 0.85, 1.0),
            );
        } else if line == "S: OK No group" {
            console.in_group = false;
            if console.active_channel == ChatChannel::Group {
                console.active_channel = ChatChannel::Room;
            }
        } else if let Some(target) = line.strip_prefix("S: OK you invited ") {
            history.add(
                console.active_channel,
                format!("[Group] Invitation sent to {}", target),
                Color::rgb(1.0, 0.85, 0.4),
            );
        } else if line == "S: ERR chat_spam_forbidden" {
            history.add(
                console.active_channel,
                "[System] Slow down! Anti-spam is active (2s).".to_string(),
                Color::rgb(1.0, 0.4, 0.4),
            );
        } else if line == "S: ERR you_have_no_group" {
            console.in_group = false;
            if console.active_channel == ChatChannel::Group {
                console.active_channel = ChatChannel::Room;
            }
            history.add(
                console.active_channel,
                "[System] You are not in a group.".to_string(),
                Color::rgb(1.0, 0.4, 0.4),
            );
        } else if line == "S: ERR player_not_found" {
            history.add(
                console.active_channel,
                "[System] Player not found.".to_string(),
                Color::rgb(1.0, 0.4, 0.4),
            );
        } else if line == "S: ERR player_already_in_group" {
            history.add(
                console.active_channel,
                "[System] That player is already in a group.".to_string(),
                Color::rgb(1.0, 0.4, 0.4),
            );
        } else if line == "S: ERR cannot_invite_yourself" {
            history.add(
                console.active_channel,
                "[System] You cannot invite yourself.".to_string(),
                Color::rgb(1.0, 0.4, 0.4),
            );
        } else if line == "S: ERR you_already_have_a_group" {
            history.add(
                console.active_channel,
                "[System] You already have a group.".to_string(),
                Color::rgb(1.0, 0.4, 0.4),
            );
        } else if line == "S: ERR no_pending_invite" {
            history.add(
                console.active_channel,
                "[System] No pending invitation.".to_string(),
                Color::rgb(1.0, 0.4, 0.4),
            );
        } else if line == "S: ERR group_no_longer_exists" {
            history.add(
                console.active_channel,
                "[System] The group no longer exists.".to_string(),
                Color::rgb(1.0, 0.4, 0.4),
            );
        }
    }
}
