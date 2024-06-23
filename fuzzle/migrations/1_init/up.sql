CREATE TABLE IF NOT EXISTS user (
    id INTEGER NOT NULL PRIMARY KEY,
    blacklist TEXT NOT NULL,
    -- both tag permissions also allow untagging stickers/sets respectively
    can_tag_stickers BOOLEAN NOT NULL CHECK (can_tag_stickers IN (0, 1)) DEFAULT 1,
    can_tag_sets BOOLEAN NOT NULL CHECK (can_tag_sets IN (0, 1)) DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    settings TEXT NULL,
    dialog_state TEXT NULL
);

CREATE TABLE IF NOT EXISTS sticker_set (
    id TEXT NOT NULL PRIMARY KEY,
    title TEXT NULL,
    last_fetched TIMESTAMP NULL DEFAULT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    added_by_user_id INTEGER NULL REFERENCES user(id) ON UPDATE RESTRICT ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS sticker_file (
    id TEXT NOT NULL PRIMARY KEY,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    tags_locked_by_user_id INTEGER NULL DEFAULT NULL,
    thumbnail_file_id TEXT NULL,
    is_animated BOOLEAN NOT NULL CHECK (is_animated IN (0, 1)) DEFAULT 1,
    FOREIGN KEY(tags_locked_by_user_id) REFERENCES user(id) ON UPDATE RESTRICT ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS sticker (
    id TEXT NOT NULL PRIMARY KEY,
    sticker_set_id TEXT NOT NULL,
    telegram_file_identifier TEXT NOT NULL,
    sticker_file_id TEXT NOT NULL,
    emoji TEXT NULL, -- some stickers don't have emojis
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(sticker_set_id) REFERENCES sticker_set(id) ON UPDATE RESTRICT ON DELETE CASCADE,
    FOREIGN KEY(sticker_file_id) REFERENCES sticker_file(id) ON UPDATE RESTRICT ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS sticker_file_tag (
    sticker_file_id TEXT NOT NULL,
    tag TEXT NOT NULL,
    added_by_user_id INTEGER NULL DEFAULT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(sticker_file_id) REFERENCES sticker_file(id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY(added_by_user_id) REFERENCES user(id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    PRIMARY KEY(sticker_file_id, tag)
);

CREATE TABLE IF NOT EXISTS potentially_similar_file (
    file_id_a TEXT NOT NULL,
    file_id_b TEXT NOT NULL,
    status INTEGER NOT NULL, -- enum
    PRIMARY KEY (file_id_a, file_id_b)
);

CREATE TABLE IF NOT EXISTS removed_set (
    id TEXT NOT NULL PRIMARY KEY,
    added_by_user_id INTEGER NULL REFERENCES user(id) ON UPDATE RESTRICT ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS sticker_file_tag_history (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    sticker_file_id TEXT NOT NULL,
    tag TEXT NOT NULL,
    removed_by_user_id INTEGER NULL, -- may be removed without user interaction (eg tag cleanup task (does not exist yet))
    added_by_user_id INTEGER NULL, -- may have been added without user interaction (import)
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(sticker_file_id) REFERENCES sticker_file(id) ON UPDATE RESTRICT ON DELETE CASCADE,
    FOREIGN KEY(removed_by_user_id) REFERENCES user(id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY(added_by_user_id) REFERENCES user(id) ON UPDATE RESTRICT ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS sticker_user (
    sticker_id TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    is_favorite BOOLEAN NOT NULL CHECK (is_favorite IN (0, 1)) DEFAULT 0, -- TODO: allow users to set favorites
    last_used TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(user_id, sticker_id),
    FOREIGN KEY(sticker_id) REFERENCES sticker(id) ON UPDATE RESTRICT ON DELETE CASCADE,
    FOREIGN KEY(user_id) REFERENCES user(id) ON UPDATE RESTRICT ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS merged_sticker (
    canonical_sticker_file_id TEXT NOT NULL,
    removed_sticker_file_id TEXT NOT NULL,
    removed_sticker_id TEXT NOT NULL,
    removed_sticker_set_id TEXT NOT NULL,
    created_by_user_id INTEGER NULL DEFAULT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(created_by_user_id) REFERENCES user(id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    PRIMARY KEY(removed_sticker_file_id, canonical_sticker_file_id, removed_sticker_id)
);

CREATE INDEX IF NOT EXISTS sticker_sticker_file_id_index ON sticker(sticker_file_id);
