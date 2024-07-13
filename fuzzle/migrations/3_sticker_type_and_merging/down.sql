ALTER TABLE sticker_file ADD COLUMN is_animated BOOLEAN NOT NULL CHECK (is_animated IN (0, 1)) DEFAULT 1,
ALTER TABLE sticker_file DROP COLUMN sticker_type;
