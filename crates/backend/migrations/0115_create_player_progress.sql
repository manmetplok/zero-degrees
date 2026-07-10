CREATE TABLE player_progress (
    device_id TEXT PRIMARY KEY,
    total_xp INTEGER NOT NULL DEFAULT 0,
    combo_count INTEGER NOT NULL DEFAULT 0,
    combo_expires_at_ms INTEGER
);
