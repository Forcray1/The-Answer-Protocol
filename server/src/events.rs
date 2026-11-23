//! Broadcast events shared between the connection tasks and the game handlers.
//!
//! Every event travels on one `tokio::sync::broadcast` channel. Each connection task
//! filters it with the `target_*` fields (see `connection.rs`):
//! `target_player` wins over `target_room`, which wins over `target_group`;
//! an event with no target goes to everybody. `sender_addr` is never sent its own event.

use std::net::SocketAddr;

use domain::{GroupId, RoomId};
use tokio::sync::broadcast::Sender;

#[derive(Clone, Debug)]
pub struct GlobalEvent {
    pub sender_addr: Option<SocketAddr>,
    pub message: String,
    pub target_room: Option<RoomId>,
    pub target_group: Option<GroupId>,
    pub target_player: Option<SocketAddr>,
}

impl GlobalEvent {
    fn new(sender_addr: Option<SocketAddr>, message: String) -> Self {
        Self {
            sender_addr,
            message,
            target_room: None,
            target_group: None,
            target_player: None,
        }
    }
}

// Send errors only mean "nobody is listening", so they are ignored on purpose.
// Argument order is (message, target) so that `message` can still borrow a value
// that the target then moves, exactly like the old struct literals did.

/// Event for one player only.
pub fn to_player(tx: &Sender<GlobalEvent>, message: String, target: SocketAddr) {
    let mut ev = GlobalEvent::new(None, message);
    ev.target_player = Some(target);
    let _ = tx.send(ev);
}

/// Event for one player, with a sender.
pub fn to_player_from(
    tx: &Sender<GlobalEvent>,
    sender: SocketAddr,
    message: String,
    target: SocketAddr,
) {
    let mut ev = GlobalEvent::new(Some(sender), message);
    ev.target_player = Some(target);
    let _ = tx.send(ev);
}

/// Event for everybody in a room.
pub fn to_room(
    tx: &Sender<GlobalEvent>,
    sender: Option<SocketAddr>,
    message: String,
    room: RoomId,
) {
    let mut ev = GlobalEvent::new(sender, message);
    ev.target_room = Some(room);
    let _ = tx.send(ev);
}

/// Event for every member of a group.
pub fn to_group(
    tx: &Sender<GlobalEvent>,
    sender: Option<SocketAddr>,
    message: String,
    group: GroupId,
) {
    let mut ev = GlobalEvent::new(sender, message);
    ev.target_group = Some(group);
    let _ = tx.send(ev);
}

/// Event for every connected player.
pub fn to_all(tx: &Sender<GlobalEvent>, sender: Option<SocketAddr>, message: String) {
    let _ = tx.send(GlobalEvent::new(sender, message));
}
