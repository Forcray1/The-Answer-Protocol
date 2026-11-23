use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::AppState;
use crate::combat::{CombatResource, CombatTransitionPhase};
use crate::command_button::{
    CommandButton, MenuCommand, spawn_command_button, update_command_button_colors,
};
use crate::game::GameState;
use crate::interaction::DialogueState;
use crate::loading::{RoomTransition, RoomTransitionRequest, TransitionPhase};
use crate::mob::{GroundItemTarget, InteractableTarget, Mob};
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::arrival_point;
use crate::ui::{ChatConsole, ChatFadeState, SelectedItem};

const WINDOW_BG: Color = Color::rgba(0.12, 0.09, 0.07, 0.97);
const WINDOW_BORDER: Color = Color::rgba(0.72, 0.58, 0.30, 1.0);
const TITLE_COLOR: Color = Color::rgba(0.90, 0.80, 0.55, 1.0);
const SECTION_COLOR: Color = Color::rgba(0.55, 0.50, 0.42, 1.0);
const REPLY_COLOR: Color = Color::rgba(0.85, 0.82, 0.75, 1.0);

/// Text shown in the answer box when there is nothing to show yet.
const DEFAULT_HINT: &str = "Click a command. The server's answer appears here.";
/// How long (seconds) after a click the server lines are copied into the answer box.
const REPLY_WINDOW: f32 = 1.5;
/// Maximum number of lines kept in the answer box.
const MAX_REPLY_LINES: usize = 8;

/// Marks the full-screen node that contains the window (hidden until N is pressed).
#[derive(Component)]
pub struct CommandMenuRoot;

/// Marks the text node that shows the server's answer.
#[derive(Component)]
pub struct MenuReplyText;

/// State of the command menu.
#[derive(Resource, Default)]
pub struct CommandMenuState {
    pub open: bool,
    /// Lines shown in the answer box.
    pub lines: Vec<String>,
    /// Server lines received before this time (in seconds) are copied into the answer box.
    pub listen_until: f32,
    /// True after a first click on QUIT: the next click really quits.
    pub quit_armed: bool,
}

impl CommandMenuState {
    /// Replaces the content of the answer box with one message.
    fn say(&mut self, message: &str) {
        self.lines = vec![message.to_string()];
    }
}

/// Builds the window once, at startup. It is hidden until the player presses N.
pub(crate) fn setup_command_menu(mut commands: Commands) {
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    position_type: PositionType::Absolute,
                    ..default()
                },
                visibility: Visibility::Hidden,
                z_index: ZIndex::Global(10),
                ..default()
            },
            CommandMenuRoot,
        ))
        .with_children(|root| {
            root.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(560.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    padding: UiRect::all(Val::Px(16.0)),
                    border: UiRect::all(Val::Px(3.0)),
                    ..default()
                },
                background_color: WINDOW_BG.into(),
                border_color: WINDOW_BORDER.into(),
                ..default()
            })
            .with_children(spawn_menu_window);
        });
}

/// Title bar, one section of buttons per kind of command, and the answer box.
fn spawn_menu_window(window: &mut ChildBuilder) {
    // Title bar with the close button.
    window
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        })
        .with_children(|bar| {
            bar.spawn(TextBundle::from_section(
                "COMMANDS  (N)",
                TextStyle {
                    font_size: 24.0,
                    color: TITLE_COLOR,
                    ..default()
                },
            ));
            spawn_command_button(bar, MenuCommand::Close);
        });

    spawn_section(
        window,
        "Look around",
        &[
            MenuCommand::Look,
            MenuCommand::Status,
            MenuCommand::Who,
            MenuCommand::Inventory,
        ],
    );
    spawn_section(
        window,
        "Move",
        &[
            MenuCommand::MoveNorth,
            MenuCommand::MoveSouth,
            MenuCommand::MoveEast,
            MenuCommand::MoveWest,
        ],
    );
    spawn_section(
        window,
        "Interact (closest item / NPC / enemy)",
        &[
            MenuCommand::Take,
            MenuCommand::Drop,
            MenuCommand::Talk,
            MenuCommand::Attack,
        ],
    );
    spawn_section(window, "Quests", &[MenuCommand::Quest, MenuCommand::Quests]);
    spawn_section(
        window,
        "Group",
        &[
            MenuCommand::GroupInfo,
            MenuCommand::GroupCreate,
            MenuCommand::GroupInvite,
            MenuCommand::GroupAccept,
            MenuCommand::GroupLeave,
        ],
    );
    spawn_section(window, "Session", &[MenuCommand::Quit]);

    // Answer box.
    window
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Px(150.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            border_color: SECTION_COLOR.into(),
            ..default()
        })
        .with_children(|reply_box| {
            reply_box.spawn((
                TextBundle::from_section(
                    DEFAULT_HINT,
                    TextStyle {
                        font_size: 16.0,
                        color: REPLY_COLOR,
                        ..default()
                    },
                ),
                MenuReplyText,
            ));
        });
}

fn spawn_section(window: &mut ChildBuilder, title: &str, commands: &[MenuCommand]) {
    window
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            ..default()
        })
        .with_children(|section| {
            section.spawn(TextBundle::from_section(
                title,
                TextStyle {
                    font_size: 14.0,
                    color: SECTION_COLOR,
                    ..default()
                },
            ));
            section
                .spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(6.0),
                        row_gap: Val::Px(6.0),
                        ..default()
                    },
                    ..default()
                })
                .with_children(|row| {
                    for &command in commands {
                        spawn_command_button(row, command);
                    }
                });
        });
}

/// Opens / closes the window with the N key.
/// It stays closed while the player types in the chat or the terminal, and during a fight.
pub(crate) fn toggle_command_menu(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<CommandMenuState>,
    console: Res<ChatConsole>,
    combat: Res<CombatResource>,
    mut root: Query<&mut Visibility, With<CommandMenuRoot>>,
) {
    let fighting = combat.in_combat || combat.phase != CombatTransitionPhase::None;
    if fighting {
        if state.open {
            state.open = false;
        }
    } else if !console.open && !console.terminal_open && keys.just_pressed(KeyCode::KeyN) {
        state.open = !state.open;
        if state.open {
            state.lines.clear();
            state.quit_armed = false;
        }
    }

    if state.is_changed() && let Ok(mut visibility) = root.get_single_mut() {
        *visibility = if state.open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Read-only game information the buttons need to decide what to do.
#[derive(SystemParam)]
pub(crate) struct MenuContext<'w> {
    sender: Res<'w, NetworkSender>,
    time: Res<'w, Time>,
    game_state: Res<'w, GameState>,
    combat: Res<'w, CombatResource>,
    dialogue: Res<'w, DialogueState>,
    transition: Res<'w, RoomTransition>,
    interactable: Res<'w, InteractableTarget>,
    ground_target: Res<'w, GroundItemTarget>,
    selected_item: Res<'w, SelectedItem>,
}

/// The chat, which GROUP INVITE opens with `/invite ` already typed.
#[derive(SystemParam)]
pub(crate) struct MenuChat<'w> {
    console: ResMut<'w, ChatConsole>,
    fade: ResMut<'w, ChatFadeState>,
}

/// Reacts to a click on any button of the menu.
pub(crate) fn handle_command_buttons(
    buttons: Query<(&Interaction, &CommandButton), Changed<Interaction>>,
    context: MenuContext,
    mut chat: MenuChat,
    mut state: ResMut<CommandMenuState>,
    mut transition_events: EventWriter<RoomTransitionRequest>,
    mobs: Query<&Mob>,
) {
    for (interaction, button) in buttons.iter() {
        if *interaction == Interaction::Pressed {
            run_command(
                button.0,
                &context,
                &mut chat,
                &mut state,
                &mut transition_events,
                &mobs,
            );
        }
    }
}

/// Does what the button `command` stands for.
fn run_command(
    command: MenuCommand,
    context: &MenuContext,
    chat: &mut MenuChat,
    state: &mut CommandMenuState,
    transition_events: &mut EventWriter<RoomTransitionRequest>,
    mobs: &Query<&Mob>,
) {
    // Any other button cancels a pending QUIT.
    if command != MenuCommand::Quit {
        state.quit_armed = false;
    }

    match command {
        MenuCommand::Close => state.open = false,

        // Commands without argument: sent as they are.
        MenuCommand::Look => send(context, state, "LOOK"),
        MenuCommand::Status => send(context, state, "STATUS"),
        MenuCommand::Who => send(context, state, "WHO"),
        MenuCommand::Inventory => send(context, state, "INVENTORY"),
        MenuCommand::Quest => send(context, state, "QUEST"),
        MenuCommand::Quests => send(context, state, "QUESTS"),
        MenuCommand::GroupInfo => send(context, state, "GROUP"),
        MenuCommand::GroupCreate => send(context, state, "GROUP CREATE"),
        MenuCommand::GroupAccept => send(context, state, "GROUP ACCEPT"),
        MenuCommand::GroupLeave => send(context, state, "GROUP LEAVE"),

        MenuCommand::MoveNorth => request_move("north", context, state, transition_events),
        MenuCommand::MoveSouth => request_move("south", context, state, transition_events),
        MenuCommand::MoveEast => request_move("east", context, state, transition_events),
        MenuCommand::MoveWest => request_move("west", context, state, transition_events),

        // TAKE: the item on the ground closest to the player.
        MenuCommand::Take => match &context.ground_target.0 {
            Some(item_id) => send(context, state, &format!("TAKE {}", item_id)),
            None => state.say("No item within reach. Walk closer to an item on the ground."),
        },

        // DROP: the item selected in the inventory window.
        MenuCommand::Drop => match &context.selected_item.id {
            Some(item_id) => send(context, state, &format!("DROP {}", item_id)),
            None => state.say("Select an item in the inventory first (I key)."),
        },

        // TALK: the closest NPC that is not an enemy.
        MenuCommand::Talk => match closest_target(context, mobs, false) {
            Some(npc_id) => send(context, state, &format!("TALK {}", npc_id)),
            None => state.say("Nobody to talk to. Walk closer to an NPC."),
        },

        // ATTACK: the closest enemy. INTERACT is what the E key sends to an enemy: it starts
        // the fight, then the combat screen sends the attacks.
        MenuCommand::Attack => match closest_target(context, mobs, true) {
            Some(enemy_id) => send(context, state, &format!("INTERACT {}", enemy_id)),
            None => state.say("No enemy within reach. Walk closer to an enemy."),
        },

        // GROUP INVITE: the chat opens with the start of the command already typed.
        MenuCommand::GroupInvite => {
            chat.console.open = true;
            chat.console.just_opened = true;
            chat.console.input_buffer = "/invite ".to_string();
            chat.fade.reset();
            state.open = false;
        }

        // QUIT: the first click only asks for a confirmation.
        MenuCommand::Quit => {
            if state.quit_armed {
                send(context, state, "QUIT");
                state.open = false;
            } else {
                state.quit_armed = true;
                state.say("Click QUIT again to leave the game.");
            }
        }
    }
}

/// Sends a command to the server and starts copying the server's answer into the menu.
fn send(context: &MenuContext, state: &mut CommandMenuState, command: &str) {
    let _ = context.sender.0.send(format!("{}\n", command));
    state.lines.clear();
    state.listen_until = context.time.elapsed_seconds() + REPLY_WINDOW;
}

/// Id of the closest NPC (`want_enemy == false`) or enemy (`want_enemy == true`), if any.
fn closest_target(context: &MenuContext, mobs: &Query<&Mob>, want_enemy: bool) -> Option<String> {
    let target_id = context.interactable.0.as_ref()?;
    mobs.iter()
        .find(|mob| &mob.npc_id == target_id && mob.is_enemy == want_enemy)
        .map(|mob| mob.npc_id.clone())
}

/// MOVE: starts the same room transition as walking to the edge of the room.
fn request_move(
    direction: &str,
    context: &MenuContext,
    state: &mut CommandMenuState,
    transition_events: &mut EventWriter<RoomTransitionRequest>,
) {
    let busy = context.combat.in_combat
        || context.combat.phase != CombatTransitionPhase::None
        || context.dialogue.is_active()
        || context.transition.phase != TransitionPhase::None;
    if busy {
        state.say("You cannot move right now.");
        return;
    }
    if !context.game_state.exits.iter().any(|exit| exit == direction) {
        state.say(&format!("There is no exit to the {}.", direction));
        return;
    }
    transition_events.send(RoomTransitionRequest {
        direction: direction.to_string(),
        arrival_point: arrival_point(&context.game_state.current_room, direction),
    });
}

/// Copies the server's answer into the menu after a click.
pub(crate) fn capture_replies(
    mut events: EventReader<ServerMessageEvent>,
    time: Res<Time>,
    mut state: ResMut<CommandMenuState>,
) {
    for event in events.read() {
        if time.elapsed_seconds() > state.listen_until {
            continue;
        }
        let line = event.0.trim();
        // `EVT` lines are updates for the other windows, not an answer to a command.
        if line.is_empty() || line.starts_with("S: EVT") {
            continue;
        }
        state.lines.push(line.strip_prefix("S: ").unwrap_or(line).to_string());
        if state.lines.len() > MAX_REPLY_LINES {
            state.lines.remove(0);
        }
    }
}

/// Writes the lines of the answer box into its text node.
pub(crate) fn refresh_reply_text(
    state: Res<CommandMenuState>,
    mut text: Query<&mut Text, With<MenuReplyText>>,
) {
    if !state.is_changed() {
        return;
    }
    if let Ok(mut text) = text.get_single_mut() {
        text.sections[0].value = if state.lines.is_empty() {
            DEFAULT_HINT.to_string()
        } else {
            state.lines.join("\n")
        };
    }
}

pub struct CommandMenuPlugin;

impl Plugin for CommandMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CommandMenuState>()
            .add_systems(Startup, setup_command_menu)
            .add_systems(
                Update,
                (
                    toggle_command_menu,
                    handle_command_buttons,
                    capture_replies,
                    refresh_reply_text,
                    update_command_button_colors,
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
