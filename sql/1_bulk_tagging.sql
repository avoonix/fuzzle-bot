-- firelex
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'firelex' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 334417239 AND sticker_set.id LIKE "%byalex") AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;

-- rbkfury
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'rbkfury' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 767361635 AND sticker_set.title LIKE "%@RBKFURY") AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;

-- niuka
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'niuka_folfsky' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 218075668 AND sticker_set.id LIKE "%_by_NiuKa") AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;

-- rainyote
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'rainyote' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 166371683) AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;

-- nowandlater
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'nowandlater' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 123869615 AND (sticker_set.id LIKE "%NaL" OR sticker_set.id LIKE "%NL")) AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;

-- pulex
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'pulex' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 117178349 AND sticker_set.id LIKE "%bypulex") AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;

-- yuniwusky
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'yuniwusky' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 110037233 AND sticker_set.id LIKE "%byyuni%") AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;

-- TODO: more taggings
