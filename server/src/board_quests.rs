// Board quest system.
//
// Notice boards ("Panneau" NPCs) in Start_oasis and Oasis2 share a single pool
// of 6 kill quests. Each quest asks a player to kill 2-10 of a specific mob and
// rewards 10-20 gold per required kill.
//
// Lifecycle:
//   available -> taken (locked to one player) -> completed (slot regenerated)
//   taken -> abandoned (slot returns to "available", counter reset)

use rand::Rng;

fn canonical_mob_key(raw: &str) -> String {
    let mut value = raw.trim().to_string();
    while value
        .chars()
        .last()
        .map(|ch| ch.is_ascii_digit())
        .unwrap_or(false)
    {
        value.pop();
    }
    value
        .replace(['-', '_', ' '], "")
        .to_ascii_lowercase()
}

/// Registry of mobs that can appear in board kill quests.
/// Fields: (mob_id, display_name, location_hint)
/// Boss-tier and special-quest mobs are intentionally excluded from the board pool.
pub static MOB_POOL: &[(&str, &str, &str)] = &[
    ("Squeletton", "Squeletton", "in the wild areas"),
    ("Squeletton1", "Squeletton", "in the wild areas"),
    ("Squeletton2", "Squeletton", "in the wild areas"),
    ("leader_Squeletton", "Squeletton Leader", "in the wild areas"),
    ("Golem1", "Golem", "in the wild areas"),
    ("Golem2", "Golem", "in the wild areas"),
    ("bandit1", "Bandit", "by the roads and ruins"),
    ("bandit2", "Bandit", "by the roads and ruins"),
];

fn is_allowed_board_mob(mob_id: &str) -> bool {
    !matches!(mob_id, "Basilic" | "Drake" | "SandMonster" | "Sandmonster")
}

/// One slot on the board: a single generated kill quest.
#[derive(Debug, Clone)]
pub struct BoardQuestSlot {
    /// 0-based index (displayed as 1-6).
    pub slot_id: usize,
    /// Mob id to kill (matches NPC id in world.yaml).
    pub mob_id: String,
    pub mob_name: String,
    /// Where the mob can be found (display hint for the player).
    pub location_hint: String,
    /// Number of kills required (2–10).
    pub kill_count: u32,
    /// Total gold reward (kill_count × gold_per_kill, where gold_per_kill between [10, 20]).
    pub gold_reward: i32,
    /// Username of the player who accepted this quest, or None if available.
    pub taken_by: Option<String>,
    /// Kills the player has completed so far.
    pub kills_done: u32,
}

impl BoardQuestSlot {
    /// Generate a new random slot.
    pub fn generate(slot_id: usize) -> Self {
        let mut rng = rand::thread_rng();
        let available_mobs: Vec<_> = MOB_POOL
            .iter()
            .copied()
            .filter(|(mob_id, _, _)| is_allowed_board_mob(mob_id))
            .collect();
        let mob = available_mobs
            .get(rng.gen_range(0..available_mobs.len()))
            .unwrap_or(&MOB_POOL[0]);
        let kill_count = rng.gen_range(2..=10_u32);
        let gold_per_kill = rng.gen_range(10..=20_i32);
        BoardQuestSlot {
            slot_id,
            mob_id: mob.0.to_string(),
            mob_name: mob.1.to_string(),
            location_hint: mob.2.to_string(),
            kill_count,
            gold_reward: gold_per_kill * kill_count as i32,
            taken_by: None,
            kills_done: 0,
        }
    }

    /// True when no player holds this slot.
    pub fn is_available(&self) -> bool {
        self.taken_by.is_none()
    }

    /// True when the mob killed by a player matches this quest slot, even when the
    /// world uses numbered variants like `Squeletton1` or `Golem2`.
    pub fn matches_mob(&self, mob_id: &str) -> bool {
        canonical_mob_key(&self.mob_id) == canonical_mob_key(mob_id)
    }

    /// readable one-line summary.
    pub fn summary(&self) -> String {
        match &self.taken_by {
            None => format!(
                "[{}] Kill {} {} {} — Reward: {} gold",
                self.slot_id + 1,
                self.kill_count,
                self.mob_name,
                self.location_hint,
                self.gold_reward,
            ),
            Some(taker) => format!(
                "[{}] Kill {} {} {} — TAKEN by {} ({}/{})",
                self.slot_id + 1,
                self.kill_count,
                self.mob_name,
                self.location_hint,
                taker,
                self.kills_done,
                self.kill_count,
            ),
        }
    }

    /// Wire format sent to the client:
    /// `<slot>:<mob_id>:<mob_name>:<kill_count>:<gold_reward>:<status>:<taker>:<kills_done>`
    pub fn to_wire(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}",
            self.slot_id + 1,
            self.mob_id,
            self.mob_name.replace(' ', "_"),
            self.kill_count,
            self.gold_reward,
            if self.taken_by.is_some() {
                "taken"
            } else {
                "available"
            },
            self.taken_by.as_deref().unwrap_or("none"),
            self.kills_done,
        )
    }
}

/// Shared board quest pool — both boards (Start_oasis and Oasis2) use the same instance.
#[derive(Debug)]
pub struct BoardQuestPool {
    pub slots: Vec<BoardQuestSlot>,
}

impl BoardQuestPool {
    /// Initialise the pool with 6 freshly generated quests.
    pub fn new() -> Self {
        let slots = (0..6).map(BoardQuestSlot::generate).collect();
        Self { slots }
    }

    /// Rebuild the board from scratch to refresh a server reset or board reset.
    pub fn refresh(&mut self) {
        *self = Self::new();
    }

    /// Return the slot currently held by `player`, if any.
    pub fn player_slot(&self, player: &str) -> Option<&BoardQuestSlot> {
        self.slots
            .iter()
            .find(|s| s.taken_by.as_deref() == Some(player))
    }

    pub fn player_slot_mut(&mut self, player: &str) -> Option<&mut BoardQuestSlot> {
        self.slots
            .iter_mut()
            .find(|s| s.taken_by.as_deref() == Some(player))
    }

    /// Wire string listing all 6 slots, pipe-separated.
    pub fn to_wire(&self) -> String {
        self.slots
            .iter()
            .map(|s| s.to_wire())
            .collect::<Vec<_>>()
            .join("|")
    }
}

impl Default for BoardQuestPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{canonical_mob_key, BoardQuestSlot};

    #[test]
    fn variants_match_same_family() {
        let slot = BoardQuestSlot {
            slot_id: 0,
            mob_id: "Squeletton".to_string(),
            mob_name: "Squeletton".to_string(),
            location_hint: "in the wild areas".to_string(),
            kill_count: 3,
            gold_reward: 30,
            taken_by: Some("alice".to_string()),
            kills_done: 0,
        };

        assert!(slot.matches_mob("Squeletton1"));
        assert!(slot.matches_mob("squeletton2"));
        assert_eq!(canonical_mob_key("Golem2"), "golem");
    }
}
