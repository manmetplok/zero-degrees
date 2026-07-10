ALTER TABLE messages ADD COLUMN draft TEXT;
ALTER TABLE messages ADD COLUMN assigned_to INTEGER REFERENCES players (id);
ALTER TABLE messages ADD COLUMN assigned_at TEXT;

CREATE TABLE assignment_notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id INTEGER NOT NULL REFERENCES messages (id),
    player_id INTEGER NOT NULL REFERENCES players (id),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
