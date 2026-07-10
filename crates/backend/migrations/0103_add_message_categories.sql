ALTER TABLE messages ADD COLUMN ai_category TEXT NOT NULL DEFAULT 'question' CHECK (ai_category IN ('billing', 'complaint', 'question', 'feedback'));
ALTER TABLE messages ADD COLUMN manual_category TEXT CHECK (manual_category IN ('billing', 'complaint', 'question', 'feedback'));
