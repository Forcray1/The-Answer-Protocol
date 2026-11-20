use std::time::Duration;
use bevy::prelude::*;
use crate::AppState;
use crate::loading::{RoomTransition, RoomTransitionRequest};
use crate::map::YSort;
use crate::net::{NetworkSender, ServerMessageEvent};

pub(crate) const SKIN_FOLDER: &str = "skin";

pub(crate) const DEFAULT_SKIN: &str = "default";

pub(crate) const DEFAULT_NAME: &str = "Player";

pub(crate) const CONNECT_OK_PREFIX: &str = "S: OK connected";

pub(crate) const DIR_FOLDERS: [&str; 4] = ["backward", "forward", "left", "right"];

pub(crate) const FRAME_COUNT: usize = 4;

pub(crate) const ANIM_FRAME_TIME: f32 = 0.12;

pub(crate) const PLAYER_SPEED: f32 = 400.0;

pub(crate) const PLAYER_RENDER_SIZE: f32 = 128.0;

pub(crate) const NAME_Y_OFFSET: f32 = 82.0;

pub(crate) const NAME_Z_OFFSET: f32 = 500.0;

pub(crate) const MAP_HALF_W: f32 = 1280.0;

pub(crate) const MAP_HALF_H: f32 = 720.0;

pub(crate) const FOOT_HALF_W: f32 = 28.0;

pub(crate) const FOOT_HALF_H: f32 = 12.0;

pub(crate) const FOOT_OFFSET_Y: f32 = -48.0;

pub(crate) const SPAWN_POINT: Vec2 = Vec2::new(-1000.0, 215.0);

pub(crate) const DEATH_RESPAWN_POINT: Vec2 = Vec2::new(-1000.0, 215.0);

pub(crate) const POS_SEND_INTERVAL: f32 = 0.1;
