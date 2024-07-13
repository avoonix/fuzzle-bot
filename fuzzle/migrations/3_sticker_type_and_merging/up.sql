ALTER TABLE sticker_file DROP COLUMN is_animated;
ALTER TABLE sticker_file ADD COLUMN sticker_type INTEGER NOT NULL DEFAULT 0;
