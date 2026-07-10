CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel TEXT NOT NULL CHECK (channel IN ('email', 'web_form', 'review', 'ticket')),
    sender TEXT NOT NULL,
    subject TEXT NOT NULL,
    body TEXT NOT NULL,
    received_at INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('open', 'cleared', 'skipped')) DEFAULT 'open',
    urgency TEXT NOT NULL CHECK (urgency IN ('low', 'normal', 'high', 'critical')),
    sentiment TEXT NOT NULL CHECK (sentiment IN ('positive', 'neutral', 'negative', 'angry')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
