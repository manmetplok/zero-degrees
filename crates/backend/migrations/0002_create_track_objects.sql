CREATE TABLE track_objects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    position REAL NOT NULL,
    link_type TEXT NOT NULL CHECK (link_type IN ('ticket', 'email', 'review', 'generic')),
    link_ref TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
