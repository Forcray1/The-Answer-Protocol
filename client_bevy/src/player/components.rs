use bevy::prelude::*;
use crate::player::config::FRAME_COUNT;

#[derive(Component)]
pub struct LocalPlayer;

#[derive(Component)]
pub struct PlayerName(pub String);

#[derive(Component)]
pub(crate) struct RemotePlayer {
    pub(crate) name: String,
    pub(crate) target: Vec2,
}

#[derive(Component)]
pub(crate) struct PlayerAnimation {
    pub(crate) textures: [[Handle<Image>; FRAME_COUNT]; 4],
    pub(crate) facing: usize,
    pub(crate) frame: usize,
    pub(crate) timer: Timer,
}

#[derive(Resource, Default)]
pub struct LocalPlayerName(pub Option<String>);

#[derive(Resource, Default)]
pub struct LocalPlayerSkin(pub String);

#[derive(Resource, Default)]
pub(crate) struct PendingDeathRespawn(pub(crate) bool);

#[derive(Resource)]
pub(crate) struct PosSendTimer(pub(crate) Timer);
