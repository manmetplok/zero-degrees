CREATE TABLE response_targets (
    urgency TEXT PRIMARY KEY CHECK (urgency IN ('critical', 'high', 'normal', 'low')),
    target_seconds INTEGER NOT NULL CHECK (target_seconds > 0)
);

INSERT INTO response_targets (urgency, target_seconds) VALUES
    ('critical', 300),
    ('high', 900),
    ('normal', 3600),
    ('low', 14400);

ALTER TABLE messages ADD COLUMN cleared_at INTEGER;
ALTER TABLE messages ADD COLUMN response_seconds INTEGER;
ALTER TABLE messages ADD COLUMN speed_bonus_awarded INTEGER;
