mod animation;
mod arrival;
mod components;
mod config;
mod movement;
mod remote;
mod spawn;

pub(crate) use animation::*;
pub(crate) use arrival::*;
pub(crate) use components::*;
pub(crate) use config::*;
pub(crate) use movement::*;
pub(crate) use remote::*;
pub(crate) use spawn::*;

use std::time::Duration;
use bevy::prelude::*;
use crate::AppState;
use crate::loading::{RoomTransition, RoomTransitionRequest};
use crate::map::YSort;
use crate::net::{NetworkSender, ServerMessageEvent};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeldDirections>()
            .init_resource::<LocalPlayerName>()
            .init_resource::<LocalPlayerSkin>()
            .init_resource::<PendingDeathRespawn>()
            .insert_resource(PosSendTimer(Timer::from_seconds(
                POS_SEND_INTERVAL,
                TimerMode::Repeating,
            )))
            .add_systems(Update, spawn_local_player_on_connect)
            .add_systems(OnEnter(AppState::MainMenu), clear_players_on_logout)
            .add_systems(
                Update,
                (
                    update_local_player,
                    send_local_position,
                    handle_presence_events,
                    update_remote_players,
                    update_player_combat_visibility,
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
