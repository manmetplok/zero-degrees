CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel TEXT NOT NULL CHECK (channel IN ('email', 'web_form', 'review', 'ticket')),
    sender TEXT NOT NULL,
    subject TEXT NOT NULL,
    body TEXT NOT NULL,
    received_at INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('open', 'cleared', 'skipped')) DEFAULT 'open',
    draft TEXT,
    reply TEXT,
    points_awarded INTEGER NOT NULL DEFAULT 0,
    resolved_at TEXT
);
