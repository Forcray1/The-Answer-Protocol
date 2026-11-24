use bevy::prelude::*;
use crate::ui::close_button::CloseButton;
use crate::ui::inventory::components::{EquipmentSlot, InventoryActionBtn, InventoryActionText, InventoryDescriptionText, InventoryDropBtn, InventoryGoldText, InventoryGrid, InventoryTab, InventoryTabBtn, InventoryUiRoot, PlayerSilhouette, StatPointsText, StatResetBtn, StatUpBtn, StatValueText};
use crate::ui::inventory::style::{BG_DARK, BG_PANEL, BORDER_GOLD, BORDER_INNER, BTN_BG, BTN_BORDER, SLOT_BG, SLOT_BORDER, STAT_GREEN, STAT_RED, TAB_ACTIVE_BG, TAB_IDLE_BG, TEXT_DIM, TEXT_LIGHT, TITLE_COLOR};

pub(crate) fn spawn_equip_slot(parent: &mut ChildBuilder, slot: &str, label: &str) {
    parent
        .spawn((
            ButtonBundle {
                style: Style {
                    width: Val::Px(70.0),
                    height: Val::Px(70.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                background_color: SLOT_BG.into(),
                border_color: SLOT_BORDER.into(),
                ..default()
            },
            EquipmentSlot(slot.to_string()),
        ))
        .with_children(|btn| {
            btn.spawn(TextBundle::from_section(
                label,
                TextStyle {
                    font_size: 11.0,
                    color: TEXT_DIM,
                    ..default()
                },
            ));
        });
}

pub(crate) fn spawn_tab_btn(parent: &mut ChildBuilder, tab: InventoryTab, active: bool) {
    let bg: Color = if active { TAB_ACTIVE_BG } else { TAB_IDLE_BG };
    parent
        .spawn((
            ButtonBundle {
                style: Style {
                    padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                    border: UiRect::bottom(Val::Px(2.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                background_color: bg.into(),
                border_color: if active { BORDER_GOLD } else { Color::NONE }.into(),
                ..default()
            },
            InventoryTabBtn(tab.clone()),
        ))
        .with_children(|btn| {
            btn.spawn(TextBundle::from_section(
                tab.label(),
                TextStyle {
                    font_size: 15.0,
                    color: if active { TITLE_COLOR } else { TEXT_DIM },
                    ..default()
                },
            ));
        });
}

/// Weapon slot | player silhouette | armor and medaillon slots.
fn spawn_equipment_area(row: &mut ChildBuilder) {
    // Left: Weapon slot
    row.spawn(NodeBundle {
        style: Style {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(8.0),
            width: Val::Px(80.0),
            ..default()
        },
        ..default()
    })
    .with_children(|col| {
        col.spawn(TextBundle::from_section(
            "Weapon",
            TextStyle {
                font_size: 11.0,
                color: TEXT_DIM,
                ..default()
            },
        ));
        spawn_equip_slot(col, "weapon", "");
    });

    // Center: player silhouette
    row.spawn(NodeBundle {
        style: Style {
            flex_grow: 1.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            height: Val::Px(200.0),
            ..default()
        },
        ..default()
    })
    .with_children(|center| {
        center.spawn((
            ImageBundle {
                style: Style {
                    width: Val::Px(128.0),
                    height: Val::Px(128.0),
                    ..default()
                },
                ..default()
            },
            PlayerSilhouette,
        ));
    });

    // Right: Armor + Medaillon
    row.spawn(NodeBundle {
        style: Style {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(12.0),
            width: Val::Px(80.0),
            ..default()
        },
        ..default()
    })
    .with_children(|col| {
        col.spawn(TextBundle::from_section(
            "Armor",
            TextStyle {
                font_size: 11.0,
                color: TEXT_DIM,
                ..default()
            },
        ));
        spawn_equip_slot(col, "chest", "");
        col.spawn(TextBundle::from_section(
            "Medaillon",
            TextStyle {
                font_size: 11.0,
                color: TEXT_DIM,
                ..default()
            },
        ));
        spawn_equip_slot(col, "medaillon", "");
    });
}

/// Equip / Use / Drop buttons and the selected item description.
fn spawn_action_bar(bar: &mut ChildBuilder) {
    bar.spawn((
        TextBundle::from_section(
            "Select an item...",
            TextStyle {
                font_size: 13.0,
                color: TEXT_LIGHT,
                ..default()
            },
        )
        .with_style(
            Style {
                max_width: Val::Px(180.0),
                ..default()
            },
        ),
        InventoryDescriptionText,
    ));
    bar.spawn((
        ButtonBundle {
            style: Style {
                width: Val::Px(110.0),
                height: Val::Px(34.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            background_color: BTN_BG.into(),
            border_color: BTN_BORDER.into(),
            ..default()
        },
        InventoryActionBtn,
    ))
    .with_children(|b| {
        b.spawn((
            TextBundle::from_section(
                "Equip",
                TextStyle {
                    font_size: 14.0,
                    color: TITLE_COLOR,
                    ..default()
                },
            ),
            InventoryActionText,
        ));
    });
    // Drop button — always shown when an item is selected
    bar.spawn((
        ButtonBundle {
            style: Style {
                width: Val::Px(80.0),
                height: Val::Px(34.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                border: UiRect::all(Val::Px(2.0)),
                margin: UiRect::left(Val::Px(6.0)),
                ..default()
            },
            background_color: Color::rgba(0.35, 0.12, 0.12, 1.0)
                .into(),
            border_color: Color::rgba(0.7, 0.2, 0.2, 1.0).into(),
            ..default()
        },
        InventoryDropBtn,
    ))
    .with_children(|b| {
        b.spawn(TextBundle::from_section(
            "Drop",
            TextStyle {
                font_size: 14.0,
                color: Color::rgb(1.0, 0.5, 0.5),
                ..default()
            },
        ));
    });
}

/// Gold, stat points and the character stats with their "+" buttons.
fn spawn_stat_block(stats: &mut ChildBuilder) {
    // Gold row
    stats
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                width: Val::Percent(100.0),
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
            ..default()
        })
        .with_children(|r| {
            r.spawn(TextBundle::from_section(
                "  Gold",
                TextStyle {
                    font_size: 14.0,
                    color: Color::rgb(1.0, 0.88, 0.3),
                    ..default()
                },
            ));
            r.spawn((
                TextBundle::from_section(
                    "0",
                    TextStyle {
                        font_size: 14.0,
                        color: Color::rgb(1.0, 0.88, 0.3),
                        ..default()
                    },
                ),
                InventoryGoldText,
            ));
        });
    // Stat points row
    stats
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                width: Val::Percent(100.0),
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
            ..default()
        })
        .with_children(|r| {
            r.spawn(TextBundle::from_section(
                "  Points",
                TextStyle {
                    font_size: 14.0,
                    color: Color::rgb(0.5, 0.8, 1.0),
                    ..default()
                },
            ));
            r.spawn((
                TextBundle::from_section(
                    "0",
                    TextStyle {
                        font_size: 14.0,
                        color: Color::rgb(0.5, 0.8, 1.0),
                        ..default()
                    },
                ),
                StatPointsText,
            ));
        });
    // Stat rows with + button
    let rows: &[(&str, Color, usize, &str)] = &[
        ("Damage", STAT_RED, 0, "damage"),
        ("Spe. Damage", STAT_RED, 1, "special_damage"),
        ("Defense", STAT_GREEN, 2, "defense"),
        ("Spe. Defense", STAT_GREEN, 3, "special_defense"),
        ("Max HP", STAT_GREEN, 4, "max_hp"),
    ];
    for (label, color, idx, stat_key) in rows {
        stats
            .spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    width: Val::Percent(100.0),
                    ..default()
                },
                ..default()
            })
            .with_children(|r| {
                r.spawn(TextBundle::from_section(
                    format!("  {}", label),
                    TextStyle {
                        font_size: 14.0,
                        color: TEXT_LIGHT,
                        ..default()
                    },
                ));
                // Value + optional + button
                r.spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(4.0),
                        ..default()
                    },
                    ..default()
                })
                .with_children(|rv| {
                    rv.spawn((
                        TextBundle::from_section(
                            "0",
                            TextStyle {
                                font_size: 14.0,
                                color: *color,
                                ..default()
                            },
                        ),
                        StatValueText(*idx),
                    ));
                    rv.spawn((
                        ButtonBundle {
                            style: Style {
                                width: Val::Px(18.0),
                                height: Val::Px(18.0),
                                align_items: AlignItems::Center,
                                justify_content:
                                    JustifyContent::Center,
                                ..default()
                            },
                            background_color: Color::rgba(
                                0.2, 0.5, 0.2, 0.0,
                            )
                            .into(), // hidden by default
                            visibility: Visibility::Hidden,
                            ..default()
                        },
                        StatUpBtn(stat_key.to_string()),
                    ))
                    .with_children(|btn| {
                        btn.spawn(TextBundle::from_section(
                            "+",
                            TextStyle {
                                font_size: 14.0,
                                color: Color::rgb(0.4, 1.0, 0.4),
                                ..default()
                            },
                        ));
                    });
                });
            });
    }
    // Reset button: refunds all the points spent above
    stats
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::FlexEnd,
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            },
            ..default()
        })
        .with_children(|r| {
            r.spawn((
                ButtonBundle {
                    style: Style {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    background_color: STAT_RESET_BG.into(),
                    ..default()
                },
                StatResetBtn,
            ))
            .with_children(|btn| {
                btn.spawn(TextBundle::from_section(
                    "Reset points",
                    TextStyle {
                        font_size: 13.0,
                        color: Color::rgb(1.0, 0.6, 0.5),
                        ..default()
                    },
                ));
            });
        });
}

const STAT_RESET_BG: Color = Color::rgba(0.5, 0.15, 0.1, 0.5);

/// Left half of the window: equipment slots, action buttons and stats.
fn spawn_left_panel(left: &mut ChildBuilder) {
    left.spawn(NodeBundle {
        style: Style {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        },
        ..default()
    })
    .with_children(spawn_equipment_area);

    left.spawn(NodeBundle {
        style: Style {
            width: Val::Percent(100.0),
            height: Val::Px(50.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(6.0)),
            border: UiRect::top(Val::Px(1.0)),
            ..default()
        },
        border_color: BORDER_INNER.into(),
        ..default()
    })
    .with_children(spawn_action_bar);

    left.spawn(NodeBundle {
        style: Style {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            padding: UiRect::horizontal(Val::Px(4.0)),
            ..default()
        },
        ..default()
    })
    .with_children(spawn_stat_block);
}

/// Right half of the window: category tabs and the item grid.
fn spawn_right_panel(right: &mut ChildBuilder) {
    right
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            border_color: BORDER_INNER.into(),
            ..default()
        })
        .with_children(|tabs| {
            spawn_tab_btn(tabs, InventoryTab::All, true);
            spawn_tab_btn(tabs, InventoryTab::Weapons, false);
            spawn_tab_btn(tabs, InventoryTab::Armor, false);
            spawn_tab_btn(tabs, InventoryTab::Medaillon, false);
            spawn_tab_btn(tabs, InventoryTab::Consumables, false);
            spawn_tab_btn(tabs, InventoryTab::Other, false);
            spawn_tab_btn(tabs, InventoryTab::KeyItems, false);
        });

    right.spawn((
        NodeBundle {
            style: Style {
                flex_grow: 1.0,
                display: Display::Grid,
                grid_template_columns: vec![GridTrack::flex(1.0); 5],
                grid_template_rows: vec![GridTrack::flex(1.0); 4],
                row_gap: Val::Px(6.0),
                column_gap: Val::Px(6.0),
                ..default()
            },
            ..default()
        },
        InventoryGrid,
    ));
}

/// Window title and close button.
fn spawn_title_bar(tb: &mut ChildBuilder) {
    tb.spawn(NodeBundle {
        style: Style {
            width: Val::Px(24.0),
            ..default()
        },
        ..default()
    });
    tb.spawn(TextBundle::from_section(
        "CHARACTER",
        TextStyle {
            font_size: 26.0,
            color: TITLE_COLOR,
            ..default()
        },
    ));
    tb.spawn((
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
        CloseButton("inventory".to_string()),
    ))
    .with_children(|b| {
        b.spawn(TextBundle::from_section(
            "X",
            TextStyle {
                font_size: 20.0,
                color: TITLE_COLOR,
                ..default()
            },
        ));
    });
}

/// Body of the window: left and right halves.
fn spawn_main_content(content: &mut ChildBuilder) {
    // LEFT PANEL — Silhouette + 3 equipment slots
    content
        .spawn(NodeBundle {
            style: Style {
                width: Val::Px(340.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(2.0)),
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            background_color: BG_PANEL.into(),
            border_color: BORDER_INNER.into(),
            ..default()
        })
        .with_children(spawn_left_panel);

    // RIGHT PANEL — Category tabs + item grid
    content
        .spawn(NodeBundle {
            style: Style {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(2.0)),
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(10.0),
                ..default()
            },
            background_color: BG_PANEL.into(),
            border_color: BORDER_INNER.into(),
            ..default()
        })
        .with_children(spawn_right_panel);
}





pub(crate) fn setup_inventory_ui(mut commands: Commands) {
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
            InventoryUiRoot,
        ))
        .with_children(|root| {
            root.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(960.0),
                    height: Val::Px(600.0),
                    flex_direction: FlexDirection::Column,
                    border: UiRect::all(Val::Px(3.0)),
                    ..default()
                },
                background_color: BG_DARK.into(),
                border_color: BORDER_GOLD.into(),
                ..default()
            })
            .with_children(|frame| {
                frame
                    .spawn(NodeBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            height: Val::Px(45.0),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            border: UiRect::bottom(Val::Px(2.0)),
                            padding: UiRect::horizontal(Val::Px(16.0)),
                            ..default()
                        },
                        background_color: Color::rgba(0.15, 0.12, 0.08, 1.0).into(),
                        border_color: BORDER_GOLD.into(),
                        ..default()
                    })
                    .with_children(spawn_title_bar);

                frame
                    .spawn(NodeBundle {
                        style: Style {
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Row,
                            padding: UiRect::all(Val::Px(14.0)),
                            column_gap: Val::Px(14.0),
                            ..default()
                        },
                        ..default()
                    })
                    .with_children(spawn_main_content);
            });
        });
}
