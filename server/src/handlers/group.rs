use std::net::SocketAddr;
use tokio::sync::broadcast::Sender;

use domain::{Group, GroupId, PlayerId};

use crate::state::ServerState;
use crate::events::{to_player_from, to_group, GlobalEvent};
use crate::handlers::util::ci_eq;

pub(crate) fn handle_group_create(
    addr: SocketAddr,
    state: &mut ServerState,
    tx: &Sender<GlobalEvent>,
) -> String {
    if let Some(player) = state.players.get(&addr) {
        if player.group.is_some() {
            return "S: ERR you_already_have_a_group\n".to_string();
        }
        let username = player.username.clone();
        let player_id = player.id.clone();

        state.group_counter += 1;
        let gid = GroupId(format!("group_{}", state.group_counter));
        let group = Group {
            id: gid.clone(),
            members: vec![player_id],
        };
        state.groups.insert(gid.clone(), group);
        if let Some(p) = state.players.get_mut(&addr) {
            p.group = Some(gid.clone());
        } else {
            return "S: ERR utilize_connect_first\n".to_string();
        }

        to_group(
            tx,
            Some(addr),
            format!("S: EVT GROUP CREATED {}\n", username),
            gid.clone(),
        );
        "S: OK group_created\n".to_string()
    } else {
        "S: ERR utilize_connect_first\n".to_string()
    }
}

pub(crate) fn handle_group_invite(
    addr: SocketAddr,
    target: String,
    state: &mut ServerState,
    tx: &Sender<GlobalEvent>,
) -> String {
    // Check that the inviter has a group
    let (gid, inviter_name) = if let Some(player) = state.players.get(&addr) {
        match &player.group {
            Some(g) => (g.clone(), player.username.clone()),
            None => return "S: ERR you_have_no_group\n".to_string(),
        }
    } else {
        return "S: ERR utilize_connect_first\n".to_string();
    };

    // Find the target player by username
    let target_addr = state
        .players
        .iter()
        .find(|(_, p)| ci_eq(&p.username, &target))
        .map(|(a, _)| *a);

    let Some(target_addr) = target_addr else {
        return "S: ERR player_not_found\n".to_string();
    };

    if target_addr == addr {
        return "S: ERR cannot_invite_yourself\n".to_string();
    }

    // Check that the target doesn't already have a group
    if let Some(target_player) = state.players.get(&target_addr) {
        if target_player.group.is_some() {
            return "S: ERR player_already_in_group\n".to_string();
        }
    }

    // Add pending group invitation
    let Some(target_player) = state.players.get(&target_addr) else {
        return "S: ERR player_not_found\n".to_string();
    };
    let target_username = target_player.username.clone();
    if let Some(target_player_mut) = state.players.get_mut(&target_addr) {
        target_player_mut.pending_group_invite = Some(gid.clone());
    } else {
        return "S: ERR player_not_found\n".to_string();
    }

    to_player_from(
        tx,
        addr,
        format!("S: EVT GROUP INVITED {} invited you to join their group (type 'group accept' to join)\n", inviter_name),
        target_addr,
    );
    format!("S: OK you invited {}\n", target_username)
}

pub(crate) fn handle_group_accept(
    addr: SocketAddr,
    state: &mut ServerState,
    tx: &Sender<GlobalEvent>,
) -> String {
    let (pending_gid, username, pid) = if let Some(player) = state.players.get(&addr) {
        if player.group.is_some() {
            return "S: ERR you_already_have_a_group\n".to_string();
        }
        match &player.pending_group_invite {
            Some(g) => (g.clone(), player.username.clone(), player.id.clone()),
            None => return "S: ERR no_pending_invite\n".to_string(),
        }
    } else {
        return "S: ERR utilize_connect_first\n".to_string();
    };

    // Check if the group still exists
    if let Some(group) = state.groups.get_mut(&pending_gid) {
        group.members.push(pid);
        if let Some(p) = state.players.get_mut(&addr) {
            p.group = Some(pending_gid.clone());
            p.pending_group_invite = None;
        } else {
            return "S: ERR utilize_connect_first\n".to_string();
        }

        to_group(
            tx,
            Some(addr),
            format!("S: EVT GROUP JOIN {}\n", username),
            pending_gid,
        );
        "S: OK you_joined_the_group\n".to_string()
    } else {
        if let Some(p) = state.players.get_mut(&addr) {
            p.pending_group_invite = None;
        }
        "S: ERR group_no_longer_exists\n".to_string()
    }
}

pub(crate) fn handle_group_leave(
    addr: SocketAddr,
    state: &mut ServerState,
    tx: &Sender<GlobalEvent>,
) -> String {
    let (gid, username) = if let Some(player) = state.players.get(&addr) {
        match &player.group {
            Some(g) => (g.clone(), player.username.clone()),
            None => return "S: ERR you_have_no_group\n".to_string(),
        }
    } else {
        return "S: ERR utilize_connect_first\n".to_string();
    };

    let player_id = PlayerId(username.clone());

    // Remove from group
    if let Some(player) = state.players.get_mut(&addr) {
        player.group = None;
    } else {
        return "S: ERR utilize_connect_first\n".to_string();
    }

    let disband = if let Some(group) = state.groups.get_mut(&gid) {
        group.members.retain(|m| m != &player_id);
        group.members.len() <= 1
    } else {
        true
    };

    if disband {
        // If 1 or 0 members remain, disband the group
        if let Some(group) = state.groups.remove(&gid) {
            for member_id in &group.members {
                // Remove group_id from the last remaining member
                if let Some((_, p)) = state.players.iter_mut().find(|(_, p)| &p.id == member_id) {
                    p.group = None;
                }
            }
        }
        to_group(
            tx,
            Some(addr),
            format!("S: EVT GROUP DISBAND {} left, group disbanded\n", username),
            gid.clone(),
        );
        "S: OK group_disbanded\n".to_string()
    } else {
        to_group(
            tx,
            Some(addr),
            format!("S: EVT GROUP LEAVE {}\n", username),
            gid,
        );
        "S: OK you_left_the_group\n".to_string()
    }
}

pub(crate) fn handle_group_info(addr: SocketAddr, state: &ServerState) -> String {
    let Some(player) = state.players.get(&addr) else {
        return "S: ERR utilize_connect_first\n".to_string();
    };
    match &player.group {
        None => "S: OK No group\n".to_string(),
        Some(gid) => {
            if let Some(group) = state.groups.get(gid) {
                let members: Vec<String> = group
                    .members
                    .iter()
                    .map(|pid| {
                        state
                            .players
                            .values()
                            .find(|p| &p.id == pid)
                            .map(|p| p.username.clone())
                            .unwrap_or_else(|| pid.0.clone())
                    })
                    .collect();
                format!("S: OK Group members: [{}]\n", members.join(", "))
            } else {
                "S: OK No group\n".to_string()
            }
        }
    }
}
