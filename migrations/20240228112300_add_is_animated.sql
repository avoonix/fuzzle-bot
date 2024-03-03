ALTER TABLE sticker_set ADD COLUMN is_animated BOOLEAN NOT NULL CHECK (is_animated IN (0, 1)) DEFAULT 0;
