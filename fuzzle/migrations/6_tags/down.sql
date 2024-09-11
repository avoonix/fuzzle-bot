ALTER TABLE tag ADD COLUMN is_pending BOOLEAN NOT NULL CHECK (is_pending IN (0, 1)) DEFAULT 1;
ALTER TABLE tag ADD COLUMN dynamic_data TEXT NULL;
ALTER TABLE tag DROP COLUMN linked_channel_id;
ALTER TABLE tag DROP COLUMN linked_user_id;
ALTER TABLE tag DROP COLUMN aliases;
ALTER TABLE tag DROP COLUMN implications;

DROP TABLE IF EXISTS moderation_task;
