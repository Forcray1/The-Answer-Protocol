use bevy::prelude::*;
use crate::net::ServerMessageEvent;

#[derive(Component)]
pub struct ChatBubble {
    pub(crate) timer: Timer,
}

pub(crate) fn spawn_chat_bubbles(
    mut commands: Commands,
    mut events: EventReader<ServerMessageEvent>,
    players: Query<(Entity, &crate::player::PlayerName)>,
    mut existing_bubbles: Query<(&Parent, &mut ChatBubble, &mut Sprite, Option<&Children>)>,
    mut texts: Query<&mut Text>,
) {
    for ev in events.read() {
        let parts: Vec<&str> = ev.0.split_whitespace().collect();
        // S: EVT ROOM CHAT <user> <msg>
        if parts.len() >= 6
            && parts[0] == "S:"
            && parts[1] == "EVT"
            && parts[2] == "ROOM"
            && parts[3] == "CHAT"
        {
            let username = parts[4];
            let message = parts[5..].join(" ");
            let text_len = message.chars().count() as f32;
            let bubble_width = (text_len * 14.0).max(50.0);

            for (player_ent, name) in &players {
                if name.0 == username {
                    let mut found = false;
                    for (parent, mut bubble, mut sprite, children) in &mut existing_bubbles {
                        if parent.get() == player_ent {
                            // Update existing background size
                            sprite.custom_size = Some(Vec2::new(bubble_width + 20.0, 40.0));
                            bubble.timer.reset();

                            // Update existing text
                            if let Some(children) = children {
                                for &child in children.iter() {
                                    if let Ok(mut text) = texts.get_mut(child) {
                                        text.sections[0].value = message.clone();
                                    }
                                }
                            }
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        commands.entity(player_ent).with_children(|p| {
                            let text_len = message.chars().count() as f32;
                            let bubble_width = (text_len * 14.0).max(50.0);

                            p.spawn((
                                SpriteBundle {
                                    sprite: Sprite {
                                        color: Color::WHITE,
                                        custom_size: Some(Vec2::new(bubble_width + 20.0, 40.0)),
                                        ..default()
                                    },
                                    transform: Transform::from_xyz(0.0, 130.0, 600.0),
                                    ..default()
                                },
                                ChatBubble {
                                    timer: Timer::from_seconds(5.0, TimerMode::Once),
                                },
                            ))
                            .with_children(|bubble_parent| {
                                bubble_parent.spawn(Text2dBundle {
                                    text: Text::from_section(
                                        message.clone(),
                                        TextStyle {
                                            font_size: 24.0,
                                            color: Color::BLACK,
                                            ..default()
                                        },
                                    )
                                    .with_justify(JustifyText::Center),
                                    transform: Transform::from_xyz(0.0, 0.0, 1.0),
                                    ..default()
                                });
                            });
                        });
                    }
                }
            }
        }
    }
}

pub(crate) fn tick_chat_bubbles(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut ChatBubble)>,
) {
    for (entity, mut bubble) in &mut query {
        bubble.timer.tick(time.delta());
        if bubble.timer.just_finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}
