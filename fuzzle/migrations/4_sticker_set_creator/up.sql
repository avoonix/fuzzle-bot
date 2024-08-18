ALTER TABLE sticker_set DROP COLUMN created_by_user_id;
ALTER TABLE sticker_set ADD COLUMN created_by_user_id INTEGER NULL;
