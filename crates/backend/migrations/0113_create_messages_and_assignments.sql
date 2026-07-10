CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel TEXT NOT NULL CHECK (channel IN ('email', 'web_form', 'review', 'ticket')),
    sender TEXT NOT NULL,
    subject TEXT NOT NULL,
    body TEXT NOT NULL,
    received_at INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('open', 'cleared', 'skipped')) DEFAULT 'open',
    draft TEXT,
    assigned_to INTEGER REFERENCES players (id),
    assigned_at TEXT
);

CREATE TABLE assignment_notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id INTEGER NOT NULL REFERENCES messages (id),
    player_id INTEGER NOT NULL REFERENCES players (id),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
