CREATE TABLE IF NOT EXISTS file_analysis (
    id TEXT NOT NULL PRIMARY KEY,
    thumbnail_file_id TEXT NULL,
    visual_hash BLOB NULL,
    histogram BLOB NULL,
    embedding BLOB NULL,
    FOREIGN KEY(id) REFERENCES file_hash(id) ON UPDATE RESTRICT ON DELETE CASCADE
);
