use bevy::prelude::*;
use crate::AppState;
use crate::combat::CombatResource;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::LocalPlayerName;
use crate::ui::chat::components::{ChatInputContainer, ChatInputPrompt, ChatInputText, ChatMessagesArea, ChatShortcutHelpText, ChatTabButton, ChatTabText, ChatText, ChatUiRoot, TerminalInputText, TerminalLogText, TerminalUiRoot};
use crate::ui::chat::state::{ChatChannel, ChatConsole, ChatFadeState};
/// One channel tab of the chat. The selected tab uses the channel color, the others are dimmed.
/// A `hidden` tab starts with `Display::None` (the Group tab until the player joins a group).
fn spawn_channel_tab(
    tabs: &mut ChildBuilder,
    channel: ChatChannel,
    label: &str,
    selected: bool,
    hidden: bool,
) {
    let (background, border, text_color) = if selected {
        (
            Color::rgba(0.20, 0.22, 0.30, 0.95),
            channel.color(),
            channel.color(),
        )
    } else {
        (
            Color::rgba(0.08, 0.08, 0.12, 0.60),
            Color::rgba(0.20, 0.20, 0.28, 0.40),
            Color::rgba(0.65, 0.65, 0.70, 0.8),
        )
    };
    tabs.spawn((
        ButtonBundle {
            style: Style {
                display: if hidden { Display::None } else { Display::Flex },
                padding: UiRect {
                    left: Val::Px(10.0),
                    right: Val::Px(10.0),
                    top: Val::Px(3.0),
                    bottom: Val::Px(3.0),
                },
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            background_color: background.into(),
            border_color: border.into(),
            ..default()
        },
        ChatTabButton(channel),
    ))
    .with_children(|btn| {
        btn.spawn((
            TextBundle::from_section(
                label,
                TextStyle {
                    font_size: 13.0,
                    color: text_color,
                    ..default()
                },
            ),
            ChatTabText(channel),
        ));
    });
}

/// Top row of the chat: the channel tabs and the shortcut help text.
fn spawn_tabs_row(tabs_row: &mut ChildBuilder) {
    tabs_row
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        })
        .with_children(|tabs| {
            spawn_channel_tab(tabs, ChatChannel::Global, "Global", true, false);
            spawn_channel_tab(tabs, ChatChannel::Room, "Room", false, false);
            // The Group tab stays hidden until the player joins a group.
            spawn_channel_tab(tabs, ChatChannel::Group, "Group", false, true);
        });

    tabs_row.spawn((
        TextBundle::from_section(
            "[Tab] Canal  [T] Parler  [F1] Console Dev",
            TextStyle {
                font_size: 11.0,
                color: Color::rgba(0.55, 0.55, 0.62, 0.7),
                ..default()
            },
        ),
        ChatShortcutHelpText,
    ));
}

/// Developer terminal (F1): header, log area and input line.
fn spawn_terminal_panel(term: &mut ChildBuilder) {
    // Header
    term.spawn(TextBundle::from_section(
        "[TERMINAL DÉVELOPPEUR] Commandes: LOOK, MOVE, ATTACK, WHO, STATUS, etc.  Fermer: [F1] / [Échap]",
        TextStyle { font_size: 13.0, color: Color::YELLOW, ..default() },
    ));

    // Log Area
    term.spawn(NodeBundle {
        style: Style {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            margin: UiRect { top: Val::Px(6.0), bottom: Val::Px(6.0), ..default() },
            padding: UiRect::all(Val::Px(8.0)),
            flex_direction: FlexDirection::ColumnReverse,
            overflow: Overflow::clip(),
            ..default()
        },
        background_color: Color::rgba(0.0, 0.0, 0.0, 0.85).into(),
        ..default()
    })
    .with_children(|log_area| {
        log_area.spawn((
            TextBundle::from_section(
                "Connected to server.\nType your commands below.\n",
                TextStyle { font_size: 14.0, color: Color::WHITE, ..default() },
            ),
            TerminalLogText,
        ));
    });

    // Input Bar
    term.spawn(NodeBundle {
        style: Style {
            width: Val::Percent(100.0),
            height: Val::Px(34.0),
            padding: UiRect { left: Val::Px(10.0), right: Val::Px(10.0), top: Val::Px(5.0), bottom: Val::Px(5.0) },
            border: UiRect::all(Val::Px(1.0)),
            align_items: AlignItems::Center,
            ..default()
        },
        background_color: Color::rgba(0.1, 0.1, 0.12, 0.95).into(),
        border_color: Color::rgba(0.4, 0.4, 0.5, 0.6).into(),
        ..default()
    })
    .with_children(|input_bar| {
        input_bar.spawn((
            TextBundle::from_section(
                "> ",
                TextStyle { font_size: 16.0, color: Color::YELLOW, ..default() },
            ),
            TerminalInputText,
        ));
    });
}




pub(crate) fn setup_chat_ui(mut commands: Commands) {
    // PLAYER CHATBOX (bottom left)
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(15.0),
                    bottom: Val::Px(15.0),
                    width: Val::Px(550.0),
                    height: Val::Px(245.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::all(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                background_color: Color::rgba(0.05, 0.06, 0.09, 0.70).into(),
                border_color: Color::rgba(0.25, 0.25, 0.35, 0.45).into(),
                visibility: Visibility::Hidden,
                z_index: ZIndex::Global(10),
                ..default()
            },
            ChatUiRoot,
            Interaction::default(),
        ))
        .with_children(|root| {
            root.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Px(26.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    margin: UiRect {
                        bottom: Val::Px(4.0),
                        ..default()
                    },
                    ..default()
                },
                ..default()
            })
            .with_children(spawn_tabs_row);

            root.spawn((
                NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        margin: UiRect {
                            top: Val::Px(2.0),
                            bottom: Val::Px(4.0),
                            ..default()
                        },
                        padding: UiRect::all(Val::Px(6.0)),
                        flex_direction: FlexDirection::ColumnReverse,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    background_color: Color::rgba(0.02, 0.03, 0.05, 0.50).into(),
                    ..default()
                },
                ChatMessagesArea,
            ))
            .with_children(|msg_area| {
                msg_area.spawn((
                    TextBundle::from_section(
                        "Welcome! Type directly to chat.\n",
                        TextStyle {
                            font_size: 14.0,
                            color: Color::rgba(0.85, 0.85, 0.90, 1.0),
                            ..default()
                        },
                    ),
                    ChatText,
                ));
            });

            root.spawn((
                ButtonBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Px(32.0),
                        padding: UiRect {
                            left: Val::Px(8.0),
                            right: Val::Px(8.0),
                            top: Val::Px(3.0),
                            bottom: Val::Px(3.0),
                        },
                        border: UiRect::all(Val::Px(1.0)),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    background_color: Color::rgba(0.04, 0.05, 0.08, 0.80).into(),
                    border_color: Color::rgba(0.25, 0.25, 0.35, 0.50).into(),
                    ..default()
                },
                ChatInputContainer,
            ))
            .with_children(|input_box| {
                input_box.spawn((
                    TextBundle::from_section(
                        "[Global] ",
                        TextStyle {
                            font_size: 14.0,
                            color: ChatChannel::Global.color(),
                            ..default()
                        },
                    ),
                    ChatInputPrompt,
                ));
                input_box.spawn((
                    TextBundle::from_section(
                        "Press 'T' to chat...",
                        TextStyle {
                            font_size: 13.0,
                            color: Color::rgba(0.55, 0.55, 0.60, 0.7),
                            ..default()
                        },
                    ),
                    ChatInputText,
                ));
            });
        });

    // DEVELOPER TERMINAL (F1 or Backquote `~`/`²`)
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    top: Val::Px(20.0),
                    left: Val::Px(20.0),
                    width: Val::Px(640.0),
                    height: Val::Px(290.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                background_color: Color::rgba(0.02, 0.02, 0.04, 0.94).into(),
                border_color: Color::rgba(0.85, 0.70, 0.20, 0.80).into(),
                visibility: Visibility::Hidden,
                z_index: ZIndex::Global(100),
                ..default()
            },
            TerminalUiRoot,
        ))
        .with_children(spawn_terminal_panel);
}

pub(crate) fn on_enter_ingame(
    mut chat_root_q: Query<&mut Visibility, (With<ChatUiRoot>, Without<TerminalUiRoot>)>,
    sender: Res<NetworkSender>,
    combat: Option<Res<CombatResource>>,
    mut fade_state: ResMut<ChatFadeState>,
) {
    let in_combat = combat.as_ref().map_or(false, |c| c.in_combat);
    if let Ok(mut vis) = chat_root_q.get_single_mut() {
        *vis = if in_combat {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    fade_state.reset();
    let _ = sender.0.send("GROUP\n".to_string());
}

pub(crate) fn on_exit_ingame(
    mut chat_root_q: Query<&mut Visibility, (With<ChatUiRoot>, Without<TerminalUiRoot>)>,
    mut term_root_q: Query<&mut Visibility, (With<TerminalUiRoot>, Without<ChatUiRoot>)>,
    mut console: ResMut<ChatConsole>,
    mut fade_state: ResMut<ChatFadeState>,
) {
    if let Ok(mut vis) = chat_root_q.get_single_mut() {
        *vis = Visibility::Hidden;
    }
    if let Ok(mut vis) = term_root_q.get_single_mut() {
        *vis = Visibility::Hidden;
    }
    console.open = false;
    console.terminal_open = false;
    console.input_buffer.clear();
    fade_state.reset();
}
