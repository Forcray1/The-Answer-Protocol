use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct PlayerStats {
    pub hp: i32,
    pub max_hp: i32,
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
