CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel TEXT NOT NULL CHECK (channel IN ('email', 'web_form', 'review', 'ticket')),
    sender TEXT NOT NULL,
    subject TEXT NOT NULL,
    body TEXT NOT NULL,
    received_at INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'cleared', 'skipped')),
    urgency TEXT NOT NULL CHECK (urgency IN ('critical', 'high', 'normal', 'low')),
    point_reward INTEGER NOT NULL,
    rationale TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
