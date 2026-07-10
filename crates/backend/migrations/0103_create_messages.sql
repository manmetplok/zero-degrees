CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel TEXT NOT NULL CHECK (channel IN ('email', 'web_form', 'review', 'ticket')),
    sender TEXT NOT NULL,
    subject TEXT NOT NULL,
    body TEXT NOT NULL,
    received_at INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('open', 'cleared', 'skipped')) DEFAULT 'open',
    ai_category TEXT NOT NULL CHECK (ai_category IN ('billing', 'complaint', 'question', 'feedback')),
    manual_category TEXT CHECK (manual_category IN ('billing', 'complaint', 'question', 'feedback')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
