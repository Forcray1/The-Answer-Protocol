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
            "QUEST" if parts.len() > 1 => GameCommand::Quest(parts[1].to_string()),
            
            "WHO" => GameCommand::Who,

            "POS" if parts.len() > 2 => match (parts[1].parse::<f32>(), parts[2].parse::<f32>()) {
                (Ok(x), Ok(y)) => GameCommand::Pos { x, y },
                _ => GameCommand::Unknown,
            },

            "QUIT" => GameCommand::Quit,
            
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