CREATE TABLE IF NOT EXISTS sticker_user (
    sticker_id TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    is_favorite BOOLEAN NOT NULL CHECK (is_favorite IN (0, 1)) DEFAULT 0, -- TODO: allow users to set favorites
    last_used DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    -- TODO: add timestamp for sorting
    PRIMARY KEY(user_id, sticker_id),
    FOREIGN KEY(sticker_id) REFERENCES sticker(id) ON UPDATE RESTRICT ON DELETE CASCADE,
    FOREIGN KEY(user_id) REFERENCES user(id) ON UPDATE RESTRICT ON DELETE CASCADE
);
