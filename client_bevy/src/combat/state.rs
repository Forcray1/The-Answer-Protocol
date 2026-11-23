use bevy::prelude::*;

pub(crate) const COMBAT_Z_BG: f32 = 2000.0;

pub(crate) const COMBAT_Z_ENTITIES: f32 = 2010.0;

pub(crate) const FADE_SPEED: f32 = 1.0;

#[derive(Resource, Default)]
pub struct CombatResource {
    pub in_combat: bool,
    pub enemy_id: String,
    pub enemy_name: String,
    pub combat_sprite: String,
    pub enemy_hp: i32,
    pub enemy_max_hp: i32,
    pub bg_index: u32,
    pub phase: CombatTransitionPhase,
    pub fade_alpha: f32,
    pub combat_message: String,
    pub message_timer: f32,
    pub bg_textures: Vec<Handle<Image>>,
    /// Which menu level is shown
    pub menu_level: MenuLevel,
    /// Cursor index within the current menu
    pub selected_action: usize,
    /// Full turn sequence state (replaces raw charge_phase)
    pub seq: TurnSequence,
    pub seq_timer: f32,
    /// Damage numbers received from server (stored until displayed)
    pub pending_dealt: i32,
    pub pending_taken: i32,
    pub pending_end: Option<String>,
    /// Player charge animation
    pub charge_phase: ChargePhase,
    /// Enemy charge animation
    pub enemy_charge_phase: ChargePhase,
    /// Heavy attack blocked turns remaining
    pub heavy_blocked_turns: u32,
    /// If true, the next player turn is an automatic stun/recharge turn.
    pub skip_next_player_turn: bool,
}

/// Sequencer for player-attack -> pause -> enemy-attack -> pause -> input.
#[derive(Default, PartialEq, Clone, Copy)]
pub enum TurnSequence {
    #[default]
    Idle, // waiting for player input
    PlayerCharging,   // player lunge animation playing
    PauseAfterPlayer, // 2 s pause showing "dealt X dmg"
    EnemyCharging,    // enemy lunge animation playing
    PauseAfterEnemy,  // 2 s pause showing "took Y dmg"
    Recovery,         // heavy-attack recovery (no anim, just text + timer)
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum CombatTransitionPhase {
    #[default]
    None,
    FadingToBlack,
    FadingFromBlack,
    ExitingToBlack,
    ExitingFromBlack,
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum ChargePhase {
    #[default]
    Idle,
    ChargingForward,
    AtPeak,
    SnapBack,
}

/// Main menu has one "Attack" entry; sub-menu has the four attack types.
#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum MenuLevel {
    #[default]
    Main, // Attack | Item | Flee
    AttackSub, // Physical | Heavy Phys | Magical | Class1 Magic
}
