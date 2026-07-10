ALTER TABLE clears ADD COLUMN xp INTEGER NOT NULL DEFAULT 0;
ALTER TABLE clears ADD COLUMN response_time_seconds INTEGER;

CREATE INDEX idx_clears_player_id ON clears (player_id);
CREATE INDEX idx_clears_cleared_at ON clears (cleared_at);
