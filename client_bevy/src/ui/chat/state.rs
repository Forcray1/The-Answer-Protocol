use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatChannel {
    #[default]
    Global,
    Room,
    Group,
}

impl ChatChannel {
    pub fn name(&self) -> &'static str {
        match self {
            ChatChannel::Global => "Global",
            ChatChannel::Room => "Room",
            ChatChannel::Group => "Group",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            ChatChannel::Global => Color::rgb(1.00, 0.85, 0.30),
            ChatChannel::Room => Color::rgb(0.35, 0.90, 0.55),
            ChatChannel::Group => Color::rgb(0.40, 0.72, 1.00),
        }
    }
}

#[derive(Resource)]
pub struct ChatConsole {
    pub open: bool,
    pub just_opened: bool,
    pub terminal_open: bool,
    pub terminal_just_opened: bool,
    pub active_channel: ChatChannel,
    pub in_group: bool,
    pub input_buffer: String,
    pub unread_global: u32,
    pub unread_room: u32,
    pub unread_group: u32,
    pub cursor_timer: Timer,
    pub cursor_visible: bool,
    pub last_sent: Option<(ChatChannel, String, std::time::Instant)>,
    pub room_change_time: Option<std::time::Instant>,
}

impl Default for ChatConsole {
    fn default() -> Self {
        Self {
            open: false,
            just_opened: false,
            terminal_open: false,
            terminal_just_opened: false,
            active_channel: ChatChannel::Global,
            in_group: false,
            input_buffer: String::new(),
            unread_global: 0,
            unread_room: 0,
            unread_group: 0,
            cursor_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
            cursor_visible: true,
            last_sent: None,
            room_change_time: None,
        }
    }
}

impl ChatConsole {
    pub fn cycle_channel(&mut self) {
        self.active_channel = match self.active_channel {
            ChatChannel::Global => ChatChannel::Room,
            ChatChannel::Room => {
                if self.in_group {
                    ChatChannel::Group
                } else {
                    ChatChannel::Global
                }
            }
            ChatChannel::Group => ChatChannel::Global,
        };
        self.clear_unread_for_active();
    }

    pub fn clear_unread_for_active(&mut self) {
        match self.active_channel {
            ChatChannel::Global => self.unread_global = 0,
            ChatChannel::Room => self.unread_room = 0,
            ChatChannel::Group => self.unread_group = 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub channel: ChatChannel,
    pub text: String,
    pub color: Color,
}

#[derive(Resource, Default)]
pub struct ChatHistory {
    pub messages: Vec<ChatMessage>,
}

impl ChatHistory {
    pub fn add(&mut self, channel: ChatChannel, text: String, color: Color) {
        self.messages.push(ChatMessage {
            channel,
            text,
            color,
        });
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
    }
}

pub const CHAT_INACTIVITY_TIMEOUT: f32 = 3.5;

pub const CHAT_FADE_DURATION: f32 = 0.8;

#[derive(Resource)]
pub struct ChatFadeState {
    pub activity_timer: f32,
    pub alpha: f32,
}

impl Default for ChatFadeState {
    fn default() -> Self {
        Self {
            activity_timer: CHAT_INACTIVITY_TIMEOUT,
            alpha: 1.0,
        }
    }
}

impl ChatFadeState {
    pub fn reset(&mut self) {
        self.activity_timer = CHAT_INACTIVITY_TIMEOUT;
        self.alpha = 1.0;
    }
}
