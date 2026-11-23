use bevy::prelude::*;

/// Marks the player sprite in the combat arena (for charge animation).
#[derive(Component)]
pub(crate) struct CombatPlayerSprite {
    pub(crate) base_x: f32,
}

/// Marks the enemy sprite in the combat arena (for charge animation).
#[derive(Component)]
pub(crate) struct CombatEnemySpritePos {
    pub(crate) base_x: f32,
}

#[derive(Component)]
pub struct CombatEntity;

#[derive(Component)]
pub(crate) struct CombatOverlay;

#[derive(Component)]
pub(crate) struct CombatEnemyVisual {
    pub(crate) timer: Timer,
    pub(crate) frame_index: usize,
    pub(crate) frames: Vec<Rect>,
}

#[derive(Component)]
pub(crate) struct EnemyHpBarFill;

#[derive(Component)]
pub(crate) struct EnemyHpText;

#[derive(Component)]
pub(crate) struct PlayerHpBarFill;

#[derive(Component)]
pub(crate) struct PlayerHpText;

#[derive(Component)]
pub(crate) struct CombatMessageText;

#[derive(Component)]
pub(crate) struct CombatButtonVisual {
    pub(crate) index: usize,
    pub(crate) current_width: f32,
    pub(crate) current_height: f32,
}

#[derive(Component)]
pub(crate) struct CombatCursorVisual {
    pub(crate) current_top: f32,
}

/// Marks the outer NodeBundle wrapper of each button slot (for show/hide).
#[derive(Component)]
pub(crate) struct CombatBtnSlot(pub(crate) usize);
