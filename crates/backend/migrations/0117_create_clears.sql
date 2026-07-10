CREATE TABLE clears (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id INTEGER NOT NULL REFERENCES players(id),
    xp INTEGER NOT NULL,
    response_time_seconds INTEGER,
    cleared_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_clears_player_id ON clears (player_id);
CREATE INDEX idx_clears_cleared_at ON clears (cleared_at);
