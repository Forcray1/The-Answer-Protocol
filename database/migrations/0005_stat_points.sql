-- Migration 0005: add unspent stat points column
ALTER TABLE players ADD COLUMN stat_points INTEGER NOT NULL DEFAULT 0;
