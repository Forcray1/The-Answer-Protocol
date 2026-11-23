use bevy::prelude::*;
use crate::ui::inventory::components::{EquipmentSlot, InventorySlotInfo, PlayerSilhouette};
use crate::ui::inventory::style::TEXT_DIM;

/// Loads the player skin sprite and shows it in the silhouette slot.
pub(crate) fn update_silhouette(
    skin: Res<crate::player::LocalPlayerSkin>,
    asset_server: Res<AssetServer>,
    mut silhouette_q: Query<&mut UiImage, With<PlayerSilhouette>>,
) {
    if !skin.is_changed() || skin.0.is_empty() {
        return;
    }
    let Ok(mut img) = silhouette_q.get_single_mut() else {
        return;
    };
    // The `backward` folder holds the sprites facing the camera.
    img.texture = asset_server.load(format!("skin/{}/backward/f1.png", skin.0));
}

pub(crate) fn handle_equip_data(
    mut commands: Commands,
    mut events: EventReader<crate::net::ServerMessageEvent>,
    asset_server: Res<AssetServer>,
    mut slots_query: Query<(Entity, &EquipmentSlot)>,
) {
    for ev in events.read() {
        if let Some(data) = ev.0.strip_prefix("S: EVT EQUIP_DATA ") {
            for item in data.trim().split('|') {
                let parts: Vec<&str> = item.split(':').collect();
                if parts.len() >= 2 {
                    let slot_name = parts[0];
                    let id = parts[1];

                    let mut is_equipped = false;
                    let mut item_name = "Vide".to_string();
                    let mut damage = 0;

                    let mut sprite_name = "none";
                    if id != "none" && parts.len() >= 4 {
                        is_equipped = true;
                        item_name = parts[2].to_string();
                        damage = parts[3].parse().unwrap_or(0);
                        if parts.len() >= 5 {
                            sprite_name = parts[4];
                        }
                    }

                    for (entity, slot_cmp) in slots_query.iter_mut() {
                        if slot_cmp.0 == slot_name {
                            // Update slot UI
                            commands.entity(entity).despawn_descendants();
                            if is_equipped {
                                commands.entity(entity).insert(InventorySlotInfo {
                                    id: id.to_string(),
                                    name: item_name.clone(),
                                    damage,
                                    slot: slot_name.to_string(),
                                    is_equipped: true,
                                    consumable: false,
                                });

                                let mut icon = None;
                                if sprite_name != "none" && !sprite_name.is_empty() {
                                    icon =
                                        Some(asset_server.load(format!("UI/{}.png", sprite_name)));
                                } else if slot_name == "weapon" {
                                    icon = Some(asset_server.load("UI/sword_icone.png"));
                                }

                                if let Some(texture) = icon {
                                    commands.entity(entity).with_children(|p| {
                                        p.spawn(ImageBundle {
                                            style: Style {
                                                width: Val::Px(50.0),
                                                height: Val::Px(50.0),
                                                ..default()
                                            },
                                            image: UiImage::new(texture),
                                            ..default()
                                        });
                                    });
                                } else {
                                    commands.entity(entity).with_children(|p| {
                                        p.spawn(TextBundle::from_section(
                                            &item_name,
                                            TextStyle {
                                                font_size: 12.0,
                                                color: Color::rgba(0.85, 0.82, 0.75, 1.0),
                                                ..default()
                                            },
                                        ));
                                    });
                                }
                            } else {
                                commands.entity(entity).remove::<InventorySlotInfo>();
                                commands.entity(entity).with_children(|p| {
                                    let label = match slot_name {
                                        "weapon" => "",
                                        "chest" => "",
                                        "medaillon" => "",
                                        _ => "",
                                    };
                                    p.spawn(TextBundle::from_section(
                                        label,
                                        TextStyle {
                                            font_size: 12.0,
                                            color: TEXT_DIM,
                                            ..default()
                                        },
                                    ));
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}
