// serveur/src/commands.rs

#[derive(Debug, PartialEq)]
pub enum GameCommand {
    Connect { username: String, password: String },
    Look,
    Move(String),
    Take(String),
    Drop(String),
    Inventory,
    Equip(String),
    Unequip(String),
    Equipment,
    Info(String),
    Attack(String),
    Status,
    Talk(String),
    Quests,
    Quest(String),
    Chat { channel: String, message: String },
    Interact(String),
    Who,
    Pos { x: f32, y: f32 },
    GroupCreate,
    GroupInvite(String),
    GroupAccept,
    GroupLeave,
    GroupInfo,
    Quit,
    Flee,
    /// Reply to a multiple-choice dialogue: `ANSWER <npc_id> <answer>`
    Answer { npc: String, answer: String },
    /// Buy an item from an NPC's shop: `BUY <npc_id> <item_id>`
    Buy { npc: String, item: String },
    /// Use a consumable item: `USE <item_id>`
    Use(String),
    /// Spend a stat point: `STATUP <stat>` (damage|special_damage|defense|special_defense|max_hp)
    StatUp(String),
    /// Magic attack: `ATTACK_MAGIC <target>`
    AttackMagic(String),
    /// Heavy physical (1.75× damage, 1-turn cooldown): `ATTACK_HEAVY <target>`
    AttackHeavy(String),
    /// Heavy magic (1.75× damage, 1-turn cooldown): `ATTACK_MAGIC_HEAVY <target>`
    AttackMagicHeavy(String),
    /// View notice board quests: `BOARD_QUESTS`
    BoardQuests,
    /// Accept a board quest by slot number: `BOARD_TAKE <1-6>`
    BoardTake(usize),
    /// Abandon the player's current board quest: `BOARD_ABANDON`
    BoardAbandon,
    Unknown,
}

impl GameCommand {
    pub fn parse(input: &str) -> Self {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return GameCommand::Unknown;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let cmd_type = parts[0].to_uppercase();

        match cmd_type.as_str() {
            "CONNECT" if parts.len() > 1 => GameCommand::Connect {
                username: parts[1].to_string(),
                password: parts.get(2).map(|s| s.to_string()).unwrap_or_default(),
            },
            "LOOK" => GameCommand::Look,
            
            "MOVE" if parts.len() > 1 => GameCommand::Move(parts[1].to_lowercase()),

            "TAKE" if parts.len() > 1 => GameCommand::Take(parts[1..].join(" ")),
            "DROP" if parts.len() > 1 => GameCommand::Drop(parts[1..].join(" ")),
            
            "INVENTORY" => GameCommand::Inventory,
            "EQUIP" if parts.len() > 1 => GameCommand::Equip(parts[1..].join(" ")),
            "UNEQUIP" if parts.len() > 1 => GameCommand::Unequip(parts[1..].join(" ")),
            "EQUIPMENT" => GameCommand::Equipment,
            "INFO" if parts.len() > 1 => GameCommand::Info(parts[1..].join(" ")),
            "STATUS" => GameCommand::Status,
            
            "ATTACK" if parts.len() > 1 => GameCommand::Attack(parts[1..].join(" ")),
            "TALK" if parts.len() > 1 => GameCommand::Talk(parts[1..].join(" ")),
            "INTERACT" if parts.len() > 1 => GameCommand::Interact(parts[1..].join(" ")),
            "QUESTS" => GameCommand::Quests,
            "QUEST" if parts.len() > 1 => GameCommand::Quest(parts[1..].join(" ")),
            
            "WHO" => GameCommand::Who,

            "POS" if parts.len() > 2 => match (parts[1].parse::<f32>(), parts[2].parse::<f32>()) {
                (Ok(x), Ok(y)) => GameCommand::Pos { x, y },
                _ => GameCommand::Unknown,
            },

            "QUIT" => GameCommand::Quit,
            "FLEE" | "ESCAPE" | "FUIR" => GameCommand::Flee,
            "ANSWER" if parts.len() > 2 => GameCommand::Answer {
                npc: parts[1].to_string(),
                answer: parts[2..].join(" ").to_lowercase(),
            },
            "USE" if parts.len() > 1 => GameCommand::Use(parts[1..].join("_")),
            "STATUP" if parts.len() > 1 => GameCommand::StatUp(parts[1].to_lowercase()),
            "ATTACK_MAGIC" if parts.len() > 1 => GameCommand::AttackMagic(parts[1..].join(" ")),
            "ATTACK_HEAVY" if parts.len() > 1 => GameCommand::AttackHeavy(parts[1..].join(" ")),
            "ATTACK_MAGIC_HEAVY" if parts.len() > 1 => GameCommand::AttackMagicHeavy(parts[1..].join(" ")),
            "BOARD_QUESTS" => GameCommand::BoardQuests,
            "BOARD_TAKE" if parts.len() > 1 => {
                if let Ok(n) = parts[1].parse::<usize>() {
                    GameCommand::BoardTake(n)
                } else {
                    GameCommand::Unknown
                }
            }
            "BOARD_ABANDON" => GameCommand::BoardAbandon,
            "BUY" if parts.len() > 2 => GameCommand::Buy {
                npc: parts[1].to_string(),
                item: parts[2].to_string(),
            },

            "CHAT" if parts.len() > 2 => {
                let channel = parts[1].to_uppercase();
                let message = parts[2..].join(" ");
                GameCommand::Chat { channel, message }
            }

            "GROUP" => {
                if parts.len() > 1 {
                    match parts[1].to_uppercase().as_str() {
                        "CREATE" => GameCommand::GroupCreate,
                        "INVITE" if parts.len() > 2 => GameCommand::GroupInvite(parts[2].to_string()),
                        "ACCEPT" => GameCommand::GroupAccept,
                        "LEAVE" => GameCommand::GroupLeave,
                        _ => GameCommand::Unknown,
                    }
                } else {
                    GameCommand::GroupInfo
                }
            }
            
            _ => GameCommand::Unknown,
        }
    }
}