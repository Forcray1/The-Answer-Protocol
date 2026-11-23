use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct InteractableTarget(pub Option<String>);

#[derive(Component)]
pub struct InteractionPrompt;

pub(crate) const MOB_FOLDER: &str = "mob";

pub(crate) const INTERACTION_PROMPT_PNJ_SPRITE: &str = "Interaction/interact_pnj.png";

pub(crate) const INTERACTION_PROMPT_MOB_SPRITE: &str = "Interaction/interact_mob.png";

pub(crate) const MOB_RENDER_SIZE: f32 = 230.0;

pub(crate) const IDLE_FRAME_TIME: f32 = 0.18;

// Spritesheet layout: 2 columns × 3 rows = 6 frames
pub(crate) const SHEET_COLS: usize = 2;

pub(crate) const SHEET_ROWS: usize = 3;

#[derive(Component)]
pub struct Mob {
    pub npc_id: String,
    pub is_enemy: bool,
}

#[derive(Component)]
pub(crate) struct MobAnimation {
    pub(crate) frames: Vec<Rect>,
    pub(crate) frame_index: usize,
    pub(crate) timer: Timer,
}

#[derive(Resource, Clone)]
pub(crate) struct InteractionPromptSprites {
    pub(crate) pnj_texture: Handle<Image>,
    pub(crate) mob_texture: Handle<Image>,
}

/// Marks an invisible board interaction point (no sprite, just a position).
#[derive(Component)]
pub struct BoardNode {
    pub npc_id: String,
}

/// A ground item rendered in the game world.
#[derive(Component)]
pub struct GroundItem {
    pub item_id: String,
}

/// The closest interactable ground item to the player, if any.
#[derive(Resource, Default)]
pub struct GroundItemTarget(pub Option<String>);

pub(crate) const GROUND_ITEM_SIZE: f32 = 48.0;

pub(crate) const GROUND_ITEM_Z: f32 = 400.0;

pub(crate) const ITEM_INTERACT_DIST: f32 = 140.0;
