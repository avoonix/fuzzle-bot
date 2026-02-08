CREATE TABLE IF NOT EXISTS banned_sticker (
    id TEXT NOT NULL PRIMARY KEY,
    telegram_file_identifier TEXT NOT NULL,
    sticker_set_id TEXT NOT NULL,
    sticker_file_id TEXT NOT NULL,
    thumbnail_file_id TEXT NULL,
    sticker_type INTEGER NOT NULL,
    clip_max_match_distance REAL NOT NULL,
    ban_reason INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
