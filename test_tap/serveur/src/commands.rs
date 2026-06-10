// serveur/src/commands.rs

#[derive(Debug, PartialEq)]
pub enum GameCommand {
    Connect(String),
    Look,
    Move(String),
    Take(String),
    Drop(String),
    Inventory,
    Attack(String),
    Status,
    Talk(String),
    Quests,
    Quest(String),
    Chat { channel: String, message: String },
    Who,
    Quit,
    Unknown,
}

impl GameCommand {
    // Analyse une ligne de texte brute reçue du client et la transforme en GameCommand
    pub fn parse(input: &str) -> Self {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return GameCommand::Unknown;
        }

        // On sépare la ligne par espaces. 
        // Exemple : "CHAT GLOBAL Hello" -> ["CHAT", "GLOBAL", "Hello"]
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let cmd_type = parts[0].to_uppercase(); // On passe en majuscules pour éviter les bugs

        match cmd_type.as_str() {
            "CONNECT" if parts.len() > 1 => GameCommand::Connect(parts[1].to_string()),
            "LOOK" => GameCommand::Look,
            
            "MOVE" if parts.len() > 1 => GameCommand::Move(parts[1].to_lowercase()),
            
            // Pour TAKE et DROP, on rassemble tout le reste des mots si l'item a un nom composé
            "TAKE" if parts.len() > 1 => GameCommand::Take(parts[1..].join(" ")),
            "DROP" if parts.len() > 1 => GameCommand::Drop(parts[1..].join(" ")),
            
            "INVENTORY" => GameCommand::Inventory,
            "STATUS" => GameCommand::Status,
            
            "ATTACK" if parts.len() > 1 => GameCommand::Attack(parts[1..].join(" ")),
            "TALK" if parts.len() > 1 => GameCommand::Talk(parts[1..].join(" ")),
            
            "QUESTS" => GameCommand::Quests,
            "QUEST" if parts.len() > 1 => GameCommand::Quest(parts[1].to_string()),
            
            "WHO" => GameCommand::Who,
            "QUIT" => GameCommand::Quit,
            
            "CHAT" if parts.len() > 2 => {
                let channel = parts[1].to_uppercase();
                let message = parts[2..].join(" ");
                GameCommand::Chat { channel, message }
            }
            
            _ => GameCommand::Unknown,
        }
    }
}