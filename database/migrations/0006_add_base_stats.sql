-- Persist base stats allocated via STATUP so they survive reconnects.
ALTER TABLE players ADD COLUMN base_damage    INTEGER NOT NULL DEFAULT 0;
ALTER TABLE players ADD COLUMN base_defense   INTEGER NOT NULL DEFAULT 0;
ALTER TABLE players ADD COLUMN base_spe_dmg   INTEGER NOT NULL DEFAULT 0;
ALTER TABLE players ADD COLUMN base_spe_def   INTEGER NOT NULL DEFAULT 0;
ALTER TABLE players ADD COLUMN base_max_hp    INTEGER NOT NULL DEFAULT 0;
