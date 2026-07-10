ALTER TABLE messages ADD COLUMN language TEXT NOT NULL DEFAULT 'en';

CREATE TABLE reply_drafts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    steering_note TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
