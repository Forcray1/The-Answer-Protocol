use bevy::prelude::*;
use crate::AppState;
use crate::combat::CombatResource;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::LocalPlayerName;

#[derive(Resource, Default)]
pub struct PlayerStats {
    pub hp: i32,
    pub max_hp: i32,
    pub xp: i32,
    pub max_xp: i32,
    pub level: i32,
}

#[derive(Component)]
pub(crate) struct HudUiRoot;

#[derive(Component)]
pub(crate) struct HpBarFill;

#[derive(Component)]
pub(crate) struct HpText;

#[derive(Component)]
pub(crate) struct XpBarFill;

#[derive(Component)]
pub(crate) struct XpText;
