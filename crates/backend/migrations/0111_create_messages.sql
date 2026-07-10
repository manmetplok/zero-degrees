CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel TEXT NOT NULL CHECK (channel IN ('email', 'web_form', 'review', 'ticket')),
    category TEXT,
    sentiment TEXT CHECK (sentiment IN ('positive', 'neutral', 'negative', 'angry')),
    status TEXT NOT NULL CHECK (status IN ('open', 'cleared', 'skipped')) DEFAULT 'open',
    received_at TEXT NOT NULL DEFAULT (datetime('now')),
    cleared_at TEXT,
    cleared_by INTEGER REFERENCES players(id)
);
