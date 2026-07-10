CREATE TABLE hazard_zones (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE hazard_zone_messages (
    zone_id INTEGER NOT NULL REFERENCES hazard_zones(id) ON DELETE CASCADE,
    message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    PRIMARY KEY (zone_id, message_id)
);
