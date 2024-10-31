-- ccart
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'ccart' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 330839721 AND sticker_set.id LIKE "ccart%") AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;

-- violet_cross
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'violet_cross' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 505581926 AND (sticker_set.title LIKE "%@Violet_Cross%" OR sticker_set.id LIKE "%_by_VCC")) AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;

-- fedu_medu
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'fedu_medu' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 224989027 AND (sticker_set.title LIKE "%by fedumedu%" OR sticker_set.title LIKE "%byfedumedu%" OR sticker_set.id LIKE "%byfm")) AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;

-- sukaridragon
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'sukaridragon' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 451469320 AND (sticker_set.title LIKE "%by SukariDragon%" OR sticker_set.title LIKE "%@SukariDragon%")) AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;

-- nix_snowsong
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'nix_snowsong' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 263912736 AND sticker_set.id LIKE "%Nix") AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;

-- reyn_goldfur
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'reyn_goldfur' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 123547509 AND (sticker_set.id LIKE "%ByReyn" OR sticker_set.title LIKE "%@ReynGoldfur%")) AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;

-- zempy3
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'zempy3' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 219081144 AND sticker_set.id LIKE "%ByZempy") AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;

-- luniquekero
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'luniquekero' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 390187266 AND sticker_set.id LIKE "%VStickers") AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;

-- lobowupp
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'lobowupp' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 119008506 AND (sticker_set.id LIKE "%bylobo%" OR sticker_set.id LIKE "%bywupp")) AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;

-- ookami_kemono
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'ookami_kemono' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 235044733) AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;
