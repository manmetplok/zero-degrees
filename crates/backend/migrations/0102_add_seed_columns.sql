ALTER TABLE messages ADD COLUMN urgency TEXT NOT NULL DEFAULT 'normal' CHECK (urgency IN ('low', 'normal', 'high', 'critical'));
ALTER TABLE messages ADD COLUMN sentiment TEXT NOT NULL DEFAULT 'neutral' CHECK (sentiment IN ('positive', 'neutral', 'negative', 'angry'));
ALTER TABLE messages ADD COLUMN created_at TEXT NOT NULL DEFAULT '';
