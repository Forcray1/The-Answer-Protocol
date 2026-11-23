use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use crate::player::LocalPlayer;
use crate::mob::components::{BoardNode, GroundItem, GroundItemTarget, INTERACTION_PROMPT_MOB_SPRITE, INTERACTION_PROMPT_PNJ_SPRITE, InteractableTarget, InteractionPrompt, InteractionPromptSprites, Mob};

pub(crate) fn setup_interaction_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    let pnj_texture = asset_server.load(INTERACTION_PROMPT_PNJ_SPRITE);
    let mob_texture = asset_server.load(INTERACTION_PROMPT_MOB_SPRITE);
    commands.insert_resource(InteractionPromptSprites {
        pnj_texture: pnj_texture.clone(),
        mob_texture,
    });

    commands.spawn((
        SpriteBundle {
            texture: pnj_texture,
            sprite: Sprite {
                custom_size: Some(Vec2::new(126.0, 126.0)),
                ..default()
            },
            transform: Transform::from_xyz(0.0, 0.0, 900.0),
            visibility: Visibility::Hidden,
            ..default()
        },
        InteractionPrompt,
    ));
}

// Named query types: they keep the system signatures short and readable
// (a long inline Query type triggers clippy::type_complexity).
type PromptQuery<'w, 's> = Query<'w, 's, (&'static mut Visibility, &'static mut Transform, &'static mut Handle<Image>), ( With<InteractionPrompt>, Without<LocalPlayer>, Without<Mob>, Without<BoardNode>, Without<GroundItem>, )>;

/// Everything the prompt can point at, grouped to keep the system's parameter list
/// short (clippy::too_many_arguments).
#[derive(SystemParam)]
pub(crate) struct InteractionTargets<'w, 's> {
    mob_q: Query<'w, 's, (&'static Transform, &'static Mob), Without<InteractionPrompt>>,
    board_q: Query<'w, 's, (&'static Transform, &'static BoardNode), Without<InteractionPrompt>>,
    items_q: Query<'w, 's, (&'static Transform, &'static GroundItem), Without<InteractionPrompt>>,
}

/// Game state that blocks or changes the prompt, grouped to keep the system's
/// parameter list short (clippy::too_many_arguments).
#[derive(SystemParam)]
pub(crate) struct InteractionContext<'w> {
    combat: Res<'w, crate::combat::CombatResource>,
    dialogue: Res<'w, crate::interaction::DialogueState>,
    prompt_sprites: Res<'w, InteractionPromptSprites>,
}

pub(crate) fn interaction_system(
    player_q: Query<&Transform, With<LocalPlayer>>,
    targets: InteractionTargets,
    mut prompt_q: PromptQuery,
    mut interactable: ResMut<InteractableTarget>,
    ground_target: Res<GroundItemTarget>,
    context: InteractionContext,
) {
    let InteractionContext {
        combat,
        dialogue,
        prompt_sprites,
    } = context;
    let InteractionTargets {
        mob_q,
        board_q,
        items_q,
    } = targets;
    if combat.in_combat || dialogue.is_active() {
        if let Ok((mut vis, _, _)) = prompt_q.get_single_mut() {
            *vis = Visibility::Hidden;
        }
        interactable.0 = None;
        return;
    }
    let Ok(player_transform) = player_q.get_single() else {
        return;
    };
    let mut closest_dist = f32::MAX;
    let mut closest_npc = None;
    let mut closest_pos = Vec3::ZERO;
    let mut closest_is_enemy = false;

    for (mob_transform, mob) in &mob_q {
        let dist = player_transform
            .translation
            .distance(mob_transform.translation);
        if dist < 150.0 && dist < closest_dist {
            closest_dist = dist;
            closest_npc = Some(mob.npc_id.clone());
            closest_pos = mob_transform.translation;
            closest_is_enemy = mob.is_enemy;
        }
    }

    // Also check board nodes (notice boards — no enemy, use PNJ prompt icon).
    for (board_transform, board) in &board_q {
        let dist = player_transform
            .translation
            .distance(board_transform.translation);
        if dist < 150.0 && dist < closest_dist {
            closest_dist = dist;
            closest_npc = Some(board.npc_id.clone());
            closest_pos = board_transform.translation;
            closest_is_enemy = false;
        }
    }

    // Show the prompt icon for nearby ground items (use PNJ bubble).
    if closest_npc.is_none() {
        if ground_target.0.is_some() {
            // Find the item entity position to float the bubble over it.
            for (tf, gi) in &items_q {
                if ground_target.0.as_deref() == Some(&gi.item_id) {
                    // Use only x/y from the item; override z so the bubble renders on top.
                    closest_pos = Vec3::new(tf.translation.x, tf.translation.y, 0.0);
                    closest_is_enemy = false;
                    break;
                }
            }
        }
    }

    let show_ground_prompt = closest_npc.is_none() && ground_target.0.is_some();
    interactable.0 = closest_npc;

    if let Ok((mut vis, mut prompt_transform, mut prompt_texture)) = prompt_q.get_single_mut() {
        if interactable.0.is_some() || show_ground_prompt {
            *vis = Visibility::Visible;
            *prompt_texture = if closest_is_enemy {
                prompt_sprites.mob_texture.clone()
            } else {
                prompt_sprites.pnj_texture.clone()
            };
            // Float above the mob / board / item
            prompt_transform.translation = closest_pos + Vec3::new(0.0, 60.0, 900.0);
        } else {
            *vis = Visibility::Hidden;
        }
    }
}

pub(crate) fn handle_interaction_input(
    input: Res<ButtonInput<KeyCode>>,
    interactable: Res<InteractableTarget>,
    ground_target: Res<GroundItemTarget>,
    combat: Res<crate::combat::CombatResource>,
    dialogue: Res<crate::interaction::DialogueState>,
    sender: Res<crate::net::NetworkSender>,
) {
    if combat.in_combat
        || combat.phase != crate::combat::CombatTransitionPhase::None
        || dialogue.is_active()
    {
        return;
    }
    if input.just_pressed(KeyCode::KeyE) {
        if let Some(ref item_id) = ground_target.0 {
            // Ground item takes priority if it is closer
            let _ = sender.0.send(format!("TAKE {}\n", item_id));
        } else if let Some(ref npc_id) = interactable.0 {
            let _ = sender.0.send(format!("INTERACT {}\n", npc_id));
        }
    }
}
