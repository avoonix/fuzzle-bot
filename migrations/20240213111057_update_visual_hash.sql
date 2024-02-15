ALTER TABLE file_analysis DROP COLUMN visual_hash;
ALTER TABLE file_analysis ADD COLUMN visual_hash BLOB NULL;
