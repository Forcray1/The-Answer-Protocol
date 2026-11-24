//! Command handlers. `process_command` dispatches a parsed command to the module that handles it.

mod board;
mod chat;
mod combat;
mod connect;
mod equip;
mod group;
mod heavy;
mod inventory;
mod magic;
mod movement;
mod npc;
mod outcome;
mod player_events;
mod quest_core;
mod quest_log;
mod quest_talk;
mod shop;
mod util;
mod wire;

pub use connect::{process_connect, save_player_to_db};
pub use wire::npc_spawn_event;

use std::net::SocketAddr;
use tokio::sync::broadcast::Sender;

use crate::commands::GameCommand;
use crate::state::ServerState;
use crate::world::WorldData;
use crate::events::GlobalEvent;
use crate::handlers::board::{handle_board_abandon, handle_board_quests, handle_board_take};
use crate::handlers::chat::{handle_chat, handle_status, handle_who};
use crate::handlers::combat::{handle_attack, handle_flee};
use crate::handlers::equip::{handle_equip, handle_unequip};
use crate::handlers::group::{handle_group_accept, handle_group_create, handle_group_info, handle_group_invite, handle_group_leave};
use crate::handlers::heavy::handle_attack_heavy;
use crate::handlers::inventory::{handle_drop, handle_equipment, handle_info, handle_inventory, handle_statreset, handle_statup, handle_take, handle_use};
use crate::handlers::magic::handle_attack_magic;
use crate::handlers::movement::{handle_look, handle_move, handle_pos};
use crate::handlers::npc::{handle_answer, handle_interact, handle_talk};
use crate::handlers::quest_log::{handle_quest, handle_quests};
use crate::handlers::shop::handle_buy;

pub fn process_command(
    addr: SocketAddr,
    command: GameCommand,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
) -> (String, bool) {
    match command {
        GameCommand::Connect { .. } => ("S: ERR already_connected\n".to_string(), false),
        GameCommand::Look => (handle_look(addr, state, world), false),
        GameCommand::Move(dir) => (handle_move(addr, dir, state, world, tx), false),
        GameCommand::Inventory => (handle_inventory(addr, state, world), false),
        GameCommand::Info(c) => (handle_info(addr, c, state, world), false),
        GameCommand::Equip(c) => (handle_equip(addr, c, state, world), false),
        GameCommand::Unequip(c) => (handle_unequip(addr, c, state, world), false),
        GameCommand::Equipment => (handle_equipment(addr, state, world), false),
        GameCommand::Take(c) => (handle_take(addr, c, state, world, tx), false),
        GameCommand::Drop(c) => (handle_drop(addr, c, state, world, tx), false),
        GameCommand::Talk(c) => (handle_talk(addr, c, state, world, tx), false),
        GameCommand::Interact(c) => (handle_interact(addr, c, state, world, tx), false),
        GameCommand::Attack(c) => (handle_attack(addr, c, state, world, tx), false),
        GameCommand::Chat { channel, message } => {
            (handle_chat(addr, channel, message, state, tx), false)
        }
        GameCommand::Who => (handle_who(addr, state), false),
        GameCommand::Pos { x, y } => (handle_pos(addr, x, y, state, tx), false),
        GameCommand::Status => (handle_status(addr, state), false),
        GameCommand::GroupCreate => (handle_group_create(addr, state, tx), false),
        GameCommand::GroupInvite(c) => (handle_group_invite(addr, c, state, tx), false),
        GameCommand::GroupAccept => (handle_group_accept(addr, state, tx), false),
        GameCommand::GroupLeave => (handle_group_leave(addr, state, tx), false),
        GameCommand::GroupInfo => (handle_group_info(addr, state), false),
        GameCommand::Quests => (handle_quests(addr, state, world), false),
        GameCommand::Quest(c) => (handle_quest(addr, c, state, world), false),
        GameCommand::Flee => (handle_flee(addr, state, tx), false),
        GameCommand::Answer { npc, answer } => {
            (handle_answer(addr, npc, answer, state, world, tx), false)
        }
        GameCommand::Buy { npc, item } => (handle_buy(addr, npc, item, state, world, tx), false),
        GameCommand::Use(item) => (handle_use(addr, item, state, world, tx), false),
        GameCommand::StatUp(stat) => (handle_statup(addr, stat, state, world), false),
        GameCommand::StatReset => (handle_statreset(addr, state, world), false),
        GameCommand::AttackMagic(target) => {
            (handle_attack_magic(addr, target, state, world, tx), false)
        }
        GameCommand::AttackHeavy(target) => (
            handle_attack_heavy(addr, target, false, state, world, tx),
            false,
        ),
        GameCommand::AttackMagicHeavy(target) => (
            handle_attack_heavy(addr, target, true, state, world, tx),
            false,
        ),
        GameCommand::BoardQuests => (handle_board_quests(addr, state), false),
        GameCommand::BoardTake(slot) => (handle_board_take(addr, slot, state, world), false),
        GameCommand::BoardAbandon => (handle_board_abandon(addr, state, world), false),
        GameCommand::Quit => ("S: OK goodbye\n".to_string(), true),
        GameCommand::Unknown => ("S: ERR malformed_command\n".to_string(), false),
    }
}
