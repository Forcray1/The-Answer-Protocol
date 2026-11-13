// serveur/src/board_quests.rs
//
// Board quest system.
//
// Notice boards ("Panneau" NPCs) in Start_oasis and Oasis2 share a single pool
// of 6 kill quests. Each quest asks a player to kill 2-10 of a specific mob and
// rewards 10-20 gold per required kill.
//
// Lifecycle:
//   available  →  taken (locked to one player)  →  completed (slot regenerated)
//   taken → abandoned (slot returns to "available", counter reset)
//
// Extending: add new mobs to MOB_POOL below and they will automatically appear
// in generated quests.

use rand::Rng;

/// Registry of mobs that can appear in board kill quests.
/// Fields: (mob_id, display_name, location_hint)
///
/// Add new rows here as new mobs are added to world.yaml.
pub static MOB_POOL: &[(&str, &str, &str)] = &[
    ("SandMonster", "Sand Monster", "in the Wild areas"),
    // Future entries:
    // ("Goblin",      "Goblin",       "in Cave1"),
    // ("Basilisk",    "Basilisk",     "in Cave4"),     ← exclude from board quests if desired
    // ("OasisSpider", "Oasis Spider", "near Oasis2"),
];

// ── Slot ─────────────────────────────────────────────────────────────────────

/// One slot on the board: a single generated kill quest.
#[derive(Debug, Clone)]
pub struct BoardQuestSlot {
    /// 0-based index (displayed as 1-6).
    pub slot_id: usize,
    /// Mob id to kill (matches NPC id in world.yaml).
    pub mob_id: String,
    /// Human-readable mob name.
    pub mob_name: String,
    /// Where the mob can be found (display hint for the player).
    pub location_hint: String,
    /// Number of kills required (2–10).
    pub kill_count: u32,
    /// Total gold reward (kill_count × gold_per_kill, where gold_per_kill ∈ [10, 20]).
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
        let mob = &MOB_POOL[rng.gen_range(0..MOB_POOL.len())];
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

    /// Human-readable one-line summary.
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

// ── Pool ─────────────────────────────────────────────────────────────────────

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
