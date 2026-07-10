ALTER TABLE messages ADD COLUMN priority TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('low', 'normal', 'high', 'critical'));
ALTER TABLE messages ADD COLUMN weight INTEGER NOT NULL DEFAULT 0;
ALTER TABLE messages ADD COLUMN cleared_by TEXT;

CREATE TABLE boss_battles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    spawned_at INTEGER NOT NULL,
    max_health INTEGER NOT NULL DEFAULT 0,
    ended_at INTEGER
);

CREATE TABLE boss_hits (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    battle_id INTEGER NOT NULL REFERENCES boss_battles(id),
    message_id INTEGER NOT NULL REFERENCES messages(id),
    runner TEXT NOT NULL,
    damage INTEGER NOT NULL,
    cleared_at INTEGER NOT NULL
);
