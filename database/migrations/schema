CREATE TABLE IF NOT EXISTS players (
    username          TEXT PRIMARY KEY,
    password_hash     TEXT NOT NULL,
    hp                INTEGER NOT NULL DEFAULT 100,
    exp               INTEGER NOT NULL DEFAULT 0,
    current_room      TEXT NOT NULL DEFAULT 'Start',
    inventory         TEXT NOT NULL DEFAULT '{}',
    equipment         TEXT NOT NULL DEFAULT '{}',
    completed_quests  TEXT NOT NULL DEFAULT '[]'
);