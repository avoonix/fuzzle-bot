CREATE TABLE IF NOT EXISTS user (
    id INTEGER NOT NULL PRIMARY KEY,
    blacklist TEXT NOT NULL,
    -- both tag permissions also allow untagging stickers/sets respectively
    can_tag_stickers BOOLEAN NOT NULL CHECK (can_tag_stickers IN (0, 1)) DEFAULT 1,
    can_tag_sets BOOLEAN NOT NULL CHECK (can_tag_sets IN (0, 1)) DEFAULT 1,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    settings TEXT NULL,
    interactions INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS sticker_set (
    id TEXT NOT NULL PRIMARY KEY,
    title TEXT NULL,
    last_fetched DATETIME NULL DEFAULT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS file_hash (
    id TEXT NOT NULL PRIMARY KEY,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    tags_locked_by_user_id INTEGER NULL DEFAULT NULL,
    FOREIGN KEY(tags_locked_by_user_id) REFERENCES user(id) ON UPDATE RESTRICT ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS sticker (
    id TEXT NOT NULL PRIMARY KEY,
    set_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    file_hash TEXT NOT NULL,
    emoji TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(set_id) REFERENCES sticker_set(id) ON UPDATE RESTRICT ON DELETE CASCADE,
    FOREIGN KEY(file_hash) REFERENCES file_hash(id) ON UPDATE RESTRICT ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS file_hash_tag (
    file_hash TEXT NOT NULL,
    tag TEXT NOT NULL,
    added_by_user_id INTEGER NULL DEFAULT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(file_hash) REFERENCES file_hash(id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY(added_by_user_id) REFERENCES user(id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    UNIQUE(file_hash, tag)
);

CREATE INDEX IF NOT EXISTS sticker_file_hash_index ON sticker(file_hash);
