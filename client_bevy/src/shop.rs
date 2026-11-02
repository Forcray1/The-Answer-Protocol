//! Shop UI — opens when the player talks to a merchant NPC.
//!
//! Protocol (server → client):
//!   `S: EVT SHOP <npc_id> <player_gold> <item_id>:<name>:<price>:<sprite>:<can_buy>|…`
//!
//! Protocol (client → server):
//!   `BUY <npc_id> <item_id>`

use bevy::prelude::*;

use crate::net::{NetworkSender, ServerMessageEvent};
use crate::AppState;

// ── Plugin ────────────────────────────────────────────────────────────────

pub struct ShopPlugin;

impl Plugin for ShopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShopState>()
            .add_systems(Startup, setup_shop_ui)
            .add_systems(
                Update,
                (
                    handle_shop_events,
                    render_shop_items,
                    handle_buy_click,
                    handle_shop_close_input,
                    handle_gold_event,
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

// ── Resource ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ShopEntry {
    pub item_id: String,
    pub name: String,
    pub price: u32,
    /// Sprite path (without `.png`).
    pub sprite: String,
    /// True if the player can buy this item right now.
    pub can_buy: bool,
}

#[derive(Resource, Default)]
pub struct ShopState {
    pub open: bool,
    pub npc_id: String,
    pub player_gold: i32,
    pub items: Vec<ShopEntry>,
    /// Set to true whenever items changed so `render_shop_items` rebuilds.
    pub dirty: bool,
}

// ── Components ────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct ShopWindowRoot;

#[derive(Component)]
struct ShopItemsContainer;

#[derive(Component)]
struct ShopGoldText;

#[derive(Component)]
struct BuyButton {
    npc_id: String,
    item_id: String,
    can_buy: bool,
}

// ── Setup ─────────────────────────────────────────────────────────────────

fn setup_shop_ui(mut commands: Commands) {
    // Root: fullscreen overlay, hidden by default
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                visibility: Visibility::Hidden,
                z_index: ZIndex::Global(8500),
                background_color: Color::rgba(0.0, 0.0, 0.0, 0.55).into(),
                ..default()
            },
            ShopWindowRoot,
        ))
        .with_children(|overlay| {
            // ── The actual shop panel ──────────────────────────────────────
            overlay
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Px(560.0),
                        max_height: Val::Px(640.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(20.0)),
                        row_gap: Val::Px(12.0),
                        overflow: Overflow::clip_y(),
                        ..default()
                    },
                    background_color: Color::rgba(0.08, 0.07, 0.05, 0.97).into(),
                    ..default()
                })
                .with_children(|panel| {
                    // ── Header row ─────────────────────────────────────────
                    panel.spawn(NodeBundle {
                        style: Style {
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            width: Val::Percent(100.0),
                            ..default()
                        },
                        ..default()
                    })
                    .with_children(|header| {
                        header.spawn(TextBundle::from_section(
                            "⚒  Shop",
                            TextStyle {
                                font_size: 32.0,
                                color: Color::rgb(1.0, 0.85, 0.3),
                                ..default()
                            },
                        ));
                        // Gold display
                        header.spawn((
                            TextBundle::from_section(
                                "0 gold",
                                TextStyle {
                                    font_size: 22.0,
                                    color: Color::rgb(1.0, 0.9, 0.3),
                                    ..default()
                                },
                            ),
                            ShopGoldText,
                        ));
                    });

                    // Divider
                    panel.spawn(NodeBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            height: Val::Px(2.0),
                            ..default()
                        },
                        background_color: Color::rgba(1.0, 0.85, 0.3, 0.4).into(),
                        ..default()
                    });

                    // ── Items container (rebuilt by render_shop_items) ──────
                    panel.spawn((
                        NodeBundle {
                            style: Style {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(8.0),
                                width: Val::Percent(100.0),
                                overflow: Overflow::clip_y(),
                                ..default()
                            },
                            ..default()
                        },
                        ShopItemsContainer,
                    ));

                    // Close hint
                    panel.spawn(TextBundle::from_section(
                        "Press E to close",
                        TextStyle {
                            font_size: 16.0,
                            color: Color::rgba(0.7, 0.7, 0.7, 0.8),
                            ..default()
                        },
                    ));
                });
        });
}

// ── Systems ───────────────────────────────────────────────────────────────

/// Parses `S: EVT SHOP` and `S: EVT GOLD` messages.
fn handle_shop_events(
    mut events: EventReader<ServerMessageEvent>,
    mut shop: ResMut<ShopState>,
    mut window_q: Query<&mut Visibility, With<ShopWindowRoot>>,
) {
    for ev in events.read() {
        if let Some(rest) = ev.0.strip_prefix("S: EVT SHOP ") {
            // Format: `<npc_id> <player_gold> <entries>`
            let mut parts = rest.splitn(3, ' ');
            let Some(npc_id) = parts.next() else { continue };
            let Some(gold_str) = parts.next() else { continue };
            let entries_str = parts.next().unwrap_or("");

            let gold: i32 = gold_str.trim().parse().unwrap_or(0);
            let items: Vec<ShopEntry> = entries_str
                .split('|')
                .filter(|s| !s.is_empty())
                .filter_map(|chunk| {
                    // item_id:name:price:sprite:can_buy
                    let mut it = chunk.splitn(5, ':');
                    let item_id = it.next()?.to_string();
                    let name    = it.next()?.to_string();
                    let price   = it.next()?.parse::<u32>().ok()?;
                    let sprite  = it.next()?.to_string();
                    let can_buy = it.next().map(|s| s.trim() == "1").unwrap_or(true);
                    Some(ShopEntry { item_id, name, price, sprite, can_buy })
                })
                .collect();

            shop.npc_id     = npc_id.to_string();
            shop.player_gold = gold;
            shop.items      = items;
            shop.open       = true;
            shop.dirty      = true;

            if let Ok(mut vis) = window_q.get_single_mut() {
                *vis = Visibility::Inherited;
            }
        }
    }
}

/// Tracks `S: EVT GOLD <amount>` to update the displayed gold without re-opening the shop.
fn handle_gold_event(
    mut events: EventReader<ServerMessageEvent>,
    mut shop: ResMut<ShopState>,
    mut gold_q: Query<&mut Text, With<ShopGoldText>>,
) {
    for ev in events.read() {
        if let Some(rest) = ev.0.strip_prefix("S: EVT GOLD ") {
            if let Ok(gold) = rest.trim().parse::<i32>() {
                shop.player_gold = gold;
                if let Ok(mut t) = gold_q.get_single_mut() {
                    t.sections[0].value = format!("{} gold", gold);
                }
            }
        }
    }
}

/// Rebuilds the item rows whenever `shop.dirty` is set.
fn render_shop_items(
    mut commands: Commands,
    mut shop: ResMut<ShopState>,
    container_q: Query<Entity, With<ShopItemsContainer>>,
    mut gold_q: Query<&mut Text, With<ShopGoldText>>,
) {
    if !shop.dirty || !shop.open { return; }
    shop.dirty = false;

    let Ok(container) = container_q.get_single() else { return };
    // Clear old children
    commands.entity(container).despawn_descendants();

    // Update gold text
    if let Ok(mut t) = gold_q.get_single_mut() {
        t.sections[0].value = format!("{} gold", shop.player_gold);
    }

    let npc_id = shop.npc_id.clone();
    let items  = shop.items.clone();

    commands.entity(container).with_children(|parent| {
        for entry in &items {
            let row_color = if entry.can_buy {
                Color::rgba(0.12, 0.10, 0.07, 0.9)
            } else {
                Color::rgba(0.08, 0.08, 0.08, 0.9)
            };

            // Row
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Px(58.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                        ..default()
                    },
                    background_color: row_color.into(),
                    ..default()
                })
                .with_children(|row| {
                    // Left: item name
                    let name_color = if entry.can_buy {
                        Color::rgb(0.95, 0.93, 0.88)
                    } else {
                        Color::rgba(0.5, 0.5, 0.5, 0.7)
                    };
                    row.spawn(TextBundle::from_section(
                        entry.name.clone(),
                        TextStyle {
                            font_size: 22.0,
                            color: name_color,
                            ..default()
                        },
                    ));

                    // Right: price + buy button
                    row.spawn(NodeBundle {
                        style: Style {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(12.0),
                            ..default()
                        },
                        ..default()
                    })
                    .with_children(|right| {
                        // Price
                        right.spawn(TextBundle::from_section(
                            format!("{} gold", entry.price),
                            TextStyle {
                                font_size: 20.0,
                                color: Color::rgb(1.0, 0.88, 0.3),
                                ..default()
                            },
                        ));

                        // Buy button
                        let btn_color = if entry.can_buy {
                            Color::rgba(0.2, 0.55, 0.2, 0.95)
                        } else {
                            Color::rgba(0.3, 0.3, 0.3, 0.7)
                        };
                        let label = if entry.can_buy { "Buy" } else { "Owned" };
                        right
                            .spawn((
                                ButtonBundle {
                                    style: Style {
                                        width: Val::Px(80.0),
                                        height: Val::Px(34.0),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    },
                                    background_color: btn_color.into(),
                                    ..default()
                                },
                                BuyButton {
                                    npc_id: npc_id.clone(),
                                    item_id: entry.item_id.clone(),
                                    can_buy: entry.can_buy,
                                },
                            ))
                            .with_children(|btn| {
                                btn.spawn(TextBundle::from_section(
                                    label,
                                    TextStyle {
                                        font_size: 18.0,
                                        color: Color::WHITE,
                                        ..default()
                                    },
                                ));
                            });
                    });
                });
        }
    });
}

/// Sends `BUY <npc_id> <item_id>` when a buy button is pressed.
fn handle_buy_click(
    mut btn_q: Query<(&Interaction, &BuyButton, &mut BackgroundColor), (Changed<Interaction>, With<Button>)>,
    sender: Option<Res<NetworkSender>>,
) {
    let Some(sender) = sender else { return };

    for (interaction, btn, mut bg) in &mut btn_q {
        if !btn.can_buy { continue; }
        match *interaction {
            Interaction::Pressed => {
                let msg = format!("BUY {} {}\n", btn.npc_id, btn.item_id);
                let _ = sender.0.try_send(msg);
            }
            Interaction::Hovered => {
                *bg = Color::rgba(0.28, 0.65, 0.28, 0.95).into();
            }
            Interaction::None => {
                *bg = Color::rgba(0.2, 0.55, 0.2, 0.95).into();
            }
        }
    }
}

/// Closes the shop when E is pressed.
fn handle_shop_close_input(
    input: Res<ButtonInput<KeyCode>>,
    mut shop: ResMut<ShopState>,
    mut window_q: Query<&mut Visibility, With<ShopWindowRoot>>,
) {
    if !shop.open { return; }
    if input.just_pressed(KeyCode::KeyE) {
        shop.open = false;
        shop.items.clear();
        if let Ok(mut vis) = window_q.get_single_mut() {
            *vis = Visibility::Hidden;
        }
    }
}
