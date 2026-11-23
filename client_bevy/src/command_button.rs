use bevy::prelude::*;

/// Every action the command menu can trigger.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuCommand {
    // Look around
    Look,
    Status,
    Who,
    Inventory,
    // Move to the next room
    MoveNorth,
    MoveSouth,
    MoveEast,
    MoveWest,
    // Interact with the closest thing
    Take,
    Drop,
    Talk,
    Attack,
    // Quests
    Quest,
    Quests,
    // Group
    GroupInfo,
    GroupCreate,
    GroupInvite,
    GroupAccept,
    GroupLeave,
    // Session
    Quit,
    // Closes the menu window (not a server command)
    Close,
}

impl MenuCommand {
    /// Text written on the button.
    pub fn label(self) -> &'static str {
        match self {
            MenuCommand::Look => "LOOK",
            MenuCommand::Status => "STATUS",
            MenuCommand::Who => "WHO",
            MenuCommand::Inventory => "INVENTORY",
            MenuCommand::MoveNorth => "MOVE NORTH",
            MenuCommand::MoveSouth => "MOVE SOUTH",
            MenuCommand::MoveEast => "MOVE EAST",
            MenuCommand::MoveWest => "MOVE WEST",
            MenuCommand::Take => "TAKE",
            MenuCommand::Drop => "DROP",
            MenuCommand::Talk => "TALK",
            MenuCommand::Attack => "ATTACK",
            MenuCommand::Quest => "QUEST",
            MenuCommand::Quests => "QUESTS",
            MenuCommand::GroupInfo => "GROUP",
            MenuCommand::GroupCreate => "GROUP CREATE",
            MenuCommand::GroupInvite => "GROUP INVITE",
            MenuCommand::GroupAccept => "GROUP ACCEPT",
            MenuCommand::GroupLeave => "GROUP LEAVE",
            MenuCommand::Quit => "QUIT",
            MenuCommand::Close => "X",
        }
    }
}

/// Marks a button of the command menu and tells which command it stands for.
#[derive(Component)]
pub struct CommandButton(pub MenuCommand);

const BUTTON_IDLE: Color = Color::rgba(0.35, 0.28, 0.18, 1.0);
const BUTTON_HOVER: Color = Color::rgba(0.48, 0.38, 0.22, 1.0);
const BUTTON_PRESSED: Color = Color::rgba(0.22, 0.17, 0.10, 1.0);
const BUTTON_BORDER: Color = Color::rgba(0.60, 0.50, 0.30, 1.0);
const BUTTON_TEXT: Color = Color::rgba(0.90, 0.80, 0.55, 1.0);

/// Spawns one button for `command` as a child of `parent`.
pub fn spawn_command_button(parent: &mut ChildBuilder, command: MenuCommand) {
    // The longer the label, the wider the button.
    let width = command.label().len() as f32 * 11.0 + 28.0;

    parent
        .spawn((
            ButtonBundle {
                style: Style {
                    width: Val::Px(width),
                    height: Val::Px(34.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                background_color: BUTTON_IDLE.into(),
                border_color: BUTTON_BORDER.into(),
                ..default()
            },
            CommandButton(command),
        ))
        .with_children(|button| {
            button.spawn(TextBundle::from_section(
                command.label(),
                TextStyle {
                    font_size: 16.0,
                    color: BUTTON_TEXT,
                    ..default()
                },
            ));
        });
}

type CommandButtonQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static mut BackgroundColor),
    (Changed<Interaction>, With<CommandButton>),
>;

pub fn update_command_button_colors(mut buttons: CommandButtonQuery) {
    for (interaction, mut background) in buttons.iter_mut() {
        let color = match *interaction {
            Interaction::Pressed => BUTTON_PRESSED,
            Interaction::Hovered => BUTTON_HOVER,
            Interaction::None => BUTTON_IDLE,
        };
        *background = color.into();
    }
}
