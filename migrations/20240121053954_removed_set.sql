CREATE TABLE IF NOT EXISTS removed_set (
    id TEXT NOT NULL PRIMARY KEY
);

CREATE TRIGGER IF NOT EXISTS prevent_insert_of_removed_set
BEFORE INSERT ON sticker_set
FOR EACH ROW
WHEN EXISTS (SELECT id FROM removed_set WHERE id = new.id)
BEGIN 

    SELECT raise(ABORT, "trying_to_insert_removed_set");
END;
