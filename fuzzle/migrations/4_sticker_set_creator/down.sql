ALTER TABLE sticker_set DROP COLUMN created_by_user_id;
ADD COLUMN created_by_user_id INTEGER NULL REFERENCES user(id) ON UPDATE RESTRICT ON DELETE RESTRICT;
