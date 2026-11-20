use std::time::Duration;
use bevy::prelude::*;
use crate::AppState;
use crate::loading::{RoomTransition, RoomTransitionRequest};
use crate::map::YSort;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::config::{MAP_HALF_H, MAP_HALF_W};

pub(crate) fn default_arrival(dir: &str) -> Vec2 {
    match dir {
        "north" => Vec2::new(0.0, -MAP_HALF_H + 5.0),
        "south" => Vec2::new(0.0, MAP_HALF_H - 5.0),
        "east" => Vec2::new(-MAP_HALF_W + 5.0, 0.0),
        "west" => Vec2::new(MAP_HALF_W - 5.0, 0.0),
        _ => Vec2::ZERO,
    }
}

pub(crate) const C_TO_W6: Vec2 = Vec2::new(-950.0, 225.0);

pub(crate) const C_TO_C: Vec2 = Vec2::new(-1255.0, 70.0);

pub(crate) const T_TO_C: Vec2 = Vec2::new(1255.0, 60.0);

pub(crate) const T4_TO_T3: Vec2 = Vec2::new(1255.0, 427.0);

pub(crate) const T3_TO_T2: Vec2 = Vec2::new(1255.0, 427.0);

pub(crate) const T2_TO_T1: Vec2 = Vec2::new(1255.0, 427.0);

pub(crate) const T1_TO_T: Vec2 = Vec2::new(1255.0, 106.0);

pub(crate) const W3_TO_O: Vec2 = Vec2::new(-1255.0, 0.0);

pub(crate) const W2_TO_O: Vec2 = Vec2::new(0.0, 720.0);

pub(crate) const W4_TO_O: Vec2 = Vec2::new(1255.0, 0.0);

pub(crate) const W6_TO_O: Vec2 = Vec2::new(0.0, -720.0);

pub(crate) fn arrival_point(from_room: &str, dir: &str) -> Vec2 {
    match (from_room, dir) {
        ("Cave1", "east") => C_TO_W6,
        ("City", "east") => C_TO_C,
        ("Castle", "west") => T_TO_C,
        ("Temple4", "west") => T4_TO_T3,
        ("Temple3", "west") => T3_TO_T2,
        ("Temple2", "west") => T2_TO_T1,
        ("Temple1", "west") => T1_TO_T,
		("Wild3", "east") => W3_TO_O,
		("Wild2", "south") => W2_TO_O,
		("Wild4", "west") => W4_TO_O,
		("Wild6", "north") => W6_TO_O,
        _ => default_arrival(dir),
    }
}
