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
