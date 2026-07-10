CREATE TABLE response_targets (
    urgency TEXT PRIMARY KEY CHECK (urgency IN ('critical', 'high', 'normal', 'low')),
    target_seconds INTEGER NOT NULL CHECK (target_seconds > 0)
);

INSERT INTO response_targets (urgency, target_seconds) VALUES
    ('critical', 300),
    ('high', 900),
    ('normal', 3600),
    ('low', 14400);

CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel TEXT NOT NULL CHECK (channel IN ('email', 'web_form', 'review', 'ticket')),
    sender TEXT NOT NULL,
    subject TEXT NOT NULL,
    body TEXT NOT NULL,
    urgency TEXT NOT NULL CHECK (urgency IN ('critical', 'high', 'normal', 'low')),
    status TEXT NOT NULL CHECK (status IN ('open', 'cleared', 'skipped')) DEFAULT 'open',
    received_at INTEGER NOT NULL,
    cleared_at INTEGER,
    response_seconds INTEGER,
    points_awarded INTEGER,
    speed_bonus_awarded INTEGER
);
