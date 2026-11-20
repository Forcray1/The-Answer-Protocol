use std::time::Duration;
use bevy::prelude::*;
use crate::AppState;
use crate::loading::{RoomTransition, RoomTransitionRequest};
use crate::map::YSort;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::components::{PlayerAnimation, PlayerName};
use crate::player::config::FRAME_COUNT;

pub(crate) fn animate(
    anim: &mut PlayerAnimation,
    texture: &mut Handle<Image>,
    dt: Duration,
    moving: bool,
    dir: Vec2,
) {
    if moving {
        anim.facing = facing_index(dir);
        anim.timer.tick(dt);
        if anim.timer.just_finished() {
            anim.frame = (anim.frame + 1) % FRAME_COUNT;
        }
    } else {
        anim.timer.reset();
        anim.frame = 0;
    }

    let wanted = anim.textures[anim.facing][anim.frame].clone();
    if *texture != wanted {
        *texture = wanted;
    }
}

pub(crate) fn facing_index(dir: Vec2) -> usize {
    if dir.x < 0.0 {
        2
    } else if dir.x > 0.0 {
        3
    } else if dir.y > 0.0 {
        1
    } else {
        0
    }
}

pub(crate) fn update_player_combat_visibility(
    combat: Res<crate::combat::CombatResource>,
    mut players: Query<(&mut Visibility, Ref<PlayerName>)>,
) {
    let combat_changed = combat.is_changed();
    if combat_changed || combat.in_combat {
        for (mut vis, name_ref) in &mut players {
            if combat_changed {
                *vis = if combat.in_combat {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                };
            } else if combat.in_combat && name_ref.is_added() {
                *vis = Visibility::Hidden;
            }
        }
    }
}
