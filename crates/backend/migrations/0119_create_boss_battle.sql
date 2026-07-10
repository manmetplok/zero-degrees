CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel TEXT NOT NULL CHECK (channel IN ('email', 'web_form', 'review', 'ticket')),
    sender TEXT NOT NULL,
    subject TEXT NOT NULL,
    priority TEXT NOT NULL CHECK (priority IN ('low', 'normal', 'high', 'critical')),
    weight INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'cleared', 'skipped')),
    received_at INTEGER NOT NULL,
    cleared_by TEXT,
    cleared_at INTEGER
);

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
