CREATE TABLE clears (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id INTEGER NOT NULL REFERENCES players(id),
    duration_seconds INTEGER NOT NULL,
    was_burning INTEGER NOT NULL DEFAULT 0,
    is_angry_aura INTEGER NOT NULL DEFAULT 0,
    is_critical INTEGER NOT NULL DEFAULT 0,
    cleared_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE day_ends (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id INTEGER NOT NULL REFERENCES players(id),
    track_empty INTEGER NOT NULL,
    ended_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE player_trophies (
    player_id INTEGER NOT NULL REFERENCES players(id),
    kind TEXT NOT NULL CHECK (kind IN ('speed_demon', 'firefighter', 'peacekeeper', 'clean_sweep', 'high_jumper')),
    tier TEXT NOT NULL CHECK (tier IN ('bronze', 'silver', 'gold')),
    first_awarded_at TEXT NOT NULL DEFAULT (datetime('now')),
    tier_awarded_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (player_id, kind)
);
