use bevy::prelude::*;
use crate::ui::chat::state::ChatChannel;

#[derive(Component)]
pub struct ChatUiRoot;

#[derive(Component)]
pub struct ChatMessagesArea;

#[derive(Component)]
pub struct ChatShortcutHelpText;

#[derive(Component)]
pub struct ChatText;

#[derive(Component)]
pub struct ChatInputPrompt;

#[derive(Component)]
pub struct ChatInputText;

#[derive(Component)]
pub struct ChatInputContainer;

#[derive(Component)]
pub struct ChatTabButton(pub ChatChannel);

#[derive(Component)]
pub struct ChatTabText(pub ChatChannel);

#[derive(Component)]
pub struct TerminalUiRoot;

#[derive(Component)]
pub struct TerminalLogText;

#[derive(Component)]
pub struct TerminalInputText;
