CREATE TABLE ai_feedback (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    feature TEXT NOT NULL CHECK (feature IN ('category', 'urgency', 'summary', 'draft_reply')),
    message_id INTEGER,
    ai_output TEXT NOT NULL,
    final_value TEXT,
    rating TEXT NOT NULL CHECK (rating IN ('helpful', 'unhelpful')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
