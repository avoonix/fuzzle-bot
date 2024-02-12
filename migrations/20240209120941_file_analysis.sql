DROP TABLE file_hash_visual_hash;
DROP TABLE visual_hash;

CREATE TABLE IF NOT EXISTS file_analysis (
    id TEXT NOT NULL PRIMARY KEY,
    thumbnail_file_id TEXT NULL,
    visual_hash TEXT NULL,
    FOREIGN KEY(id) REFERENCES file_hash(id) ON UPDATE RESTRICT ON DELETE CASCADE
);
