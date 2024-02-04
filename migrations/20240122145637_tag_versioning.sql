CREATE TABLE IF NOT EXISTS file_hash_tag_history (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    file_hash TEXT NOT NULL,
    tag TEXT NOT NULL,
    removed_by_user_id INTEGER NULL, -- may be removed without user interaction (eg tag cleanup task (does not exist yet))
    added_by_user_id INTEGER NULL, -- may have been added without user interaction (import)
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(file_hash) REFERENCES file_hash(id) ON UPDATE RESTRICT ON DELETE CASCADE,
    FOREIGN KEY(removed_by_user_id) REFERENCES user(id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY(added_by_user_id) REFERENCES user(id) ON UPDATE RESTRICT ON DELETE RESTRICT
);

CREATE TRIGGER IF NOT EXISTS delete_file_hash_tag
AFTER INSERT ON file_hash_tag_history
FOR EACH ROW
BEGIN
    DELETE FROM file_hash_tag WHERE file_hash = new.file_hash AND tag = new.tag;
END;
