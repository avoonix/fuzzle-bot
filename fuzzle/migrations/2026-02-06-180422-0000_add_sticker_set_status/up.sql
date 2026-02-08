ALTER TABLE sticker_set ADD COLUMN is_pending BOOLEAN NOT NULL CHECK (is_pending IN (0, 1)) DEFAULT 1;
 