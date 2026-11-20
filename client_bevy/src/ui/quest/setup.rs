use bevy::prelude::*;
use crate::AppState;
use crate::combat::CombatResource;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::LocalPlayerName;
use crate::ui::close_button::CloseButton;
use crate::ui::quest::components::{QUEST_BG, QUEST_BORDER, QUEST_DETAIL_TEXT, QUEST_DETAIL_TITLE, QUEST_PARCHMENT, QUEST_TITLE_COLOR, QuestDetailDescription, QuestDetailObjective, QuestDetailTitle, QuestEmptyText, QuestListPanel, QuestUiRoot};
/// Left side of the journal: the list of quests.
fn spawn_quest_list_panel(left: &mut ChildBuilder) {
    // Quest list container
    left.spawn((
        NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            ..default()
        },
        QuestListPanel,
    ));

    // Empty state message
    left.spawn((
        TextBundle::from_section(
            "No active quests",
            TextStyle {
                font_size: 18.0,
                color: Color::rgba(0.6, 0.55, 0.4, 0.7),
                ..default()
            },
        )
        .with_style(Style {
            margin: UiRect::top(Val::Px(20.0)),
            align_self: AlignSelf::Center,
            ..default()
        }),
        QuestEmptyText,
    ));
}

/// Right side of the journal: title, description and objective of the selected quest.
fn spawn_quest_detail_panel(right: &mut ChildBuilder) {
    // Quest Title
    right.spawn((
        TextBundle::from_section(
            "Select a quest",
            TextStyle {
                font_size: 24.0,
                color: QUEST_DETAIL_TITLE,
                ..default()
            },
        ),
        QuestDetailTitle,
    ));

    // Separator
    right.spawn(NodeBundle {
        style: Style {
            width: Val::Percent(100.0),
            height: Val::Px(2.0),
            ..default()
        },
        background_color: Color::rgba(0.5, 0.4, 0.25, 0.5).into(),
        ..default()
    });

    // Description label + text
    right
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            ..default()
        })
        .with_children(|desc_block| {
            desc_block.spawn(TextBundle::from_section(
                "Description",
                TextStyle {
                    font_size: 16.0,
                    color: Color::rgba(0.4, 0.35, 0.25, 0.8),
                    ..default()
                },
            ));
            desc_block.spawn((
                TextBundle::from_section(
                    "",
                    TextStyle {
                        font_size: 18.0,
                        color: QUEST_DETAIL_TEXT,
                        ..default()
                    },
                ),
                QuestDetailDescription,
            ));
        });

    // Objective label + text
    right
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            ..default()
        })
        .with_children(|obj_block| {
            obj_block.spawn(TextBundle::from_section(
                "Objective",
                TextStyle {
                    font_size: 16.0,
                    color: Color::rgba(0.4, 0.35, 0.25, 0.8),
                    ..default()
                },
            ));
            obj_block.spawn((
                TextBundle::from_section(
                    "",
                    TextStyle {
                        font_size: 18.0,
                        color: Color::rgba(0.45, 0.30, 0.12, 1.0),
                        ..default()
                    },
                ),
                QuestDetailObjective,
            ));
        });
}

/// Body of the journal: list on the left, details on the right.
fn spawn_quest_split(split: &mut ChildBuilder) {
    split
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(38.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip_y(),
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            border_color: Color::rgba(0.4, 0.35, 0.2, 0.5).into(),
            ..default()
        })
        .with_children(spawn_quest_list_panel);

    split
        .spawn(NodeBundle {
            style: Style {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(20.0)),
                border: UiRect::all(Val::Px(2.0)),
                row_gap: Val::Px(16.0),
                ..default()
            },
            background_color: QUEST_PARCHMENT.into(),
            border_color: Color::rgba(0.6, 0.5, 0.3, 0.6).into(),
            ..default()
        })
        .with_children(spawn_quest_detail_panel);
}

/// Journal title and close button.
fn spawn_quest_header(header: &mut ChildBuilder) {
    header.spawn(NodeBundle {
        style: Style {
            width: Val::Px(24.0),
            ..default()
        },
        ..default()
    });
    header.spawn(TextBundle::from_section(
        "Quest Journal",
        TextStyle {
            font_size: 28.0,
            color: QUEST_TITLE_COLOR,
            ..default()
        },
    ));
    header
        .spawn((
            ButtonBundle {
                style: Style {
                    width: Val::Px(24.0),
                    height: Val::Px(24.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                background_color: Color::NONE.into(),
                ..default()
            },
            CloseButton("quests".to_string()),
        ))
        .with_children(|btn| {
            btn.spawn(TextBundle::from_section(
                "X",
                TextStyle {
                    font_size: 24.0,
                    color: QUEST_TITLE_COLOR,
                    ..default()
                },
            ));
        });
}

/// The journal frame: header and body.
fn spawn_quest_window(window: &mut ChildBuilder) {
    window
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::new(
                    Val::Px(12.0),
                    Val::Px(12.0),
                    Val::Px(8.0),
                    Val::Px(12.0),
                ),
                border: UiRect::bottom(Val::Px(2.0)),
                margin: UiRect::bottom(Val::Px(12.0)),
                ..default()
            },
            border_color: QUEST_BORDER.into(),
            ..default()
        })
        .with_children(spawn_quest_header);

    window
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(12.0),
                ..default()
            },
            ..default()
        })
        .with_children(spawn_quest_split);
}


pub(crate) fn setup_quest_ui(mut commands: Commands) {
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
            QuestUiRoot,
        ))
        .with_children(|root| {
            root.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(780.0),
                    height: Val::Px(520.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(16.0)),
                    border: UiRect::all(Val::Px(3.0)),
                    ..default()
                },
                background_color: QUEST_BG.into(),
                border_color: QUEST_BORDER.into(),
                ..default()
            })
            .with_children(spawn_quest_window);
        });
}
