-- Category: Artist (1)
INSERT INTO tag (id, category, linked_channel_id, linked_user_id) VALUES ('acziever', 1, -1002228677003, 1059310079);

-- auto tagging
INSERT INTO sticker_file_tag (sticker_file_id, tag) SELECT DISTINCT sticker_file_id, 'acziever' FROM sticker WHERE sticker.sticker_set_id IN (SELECT sticker_set.id FROM sticker_set WHERE sticker_set.created_by_user_id = 1059310079 AND sticker_set.id LIKE '%_0_Aczi') AND NOT EXISTS (SELECT * FROM sticker_file WHERE sticker.sticker_file_id = sticker_file.id AND sticker_file.tags_locked_by_user_id IS NOT NULL) ON CONFLICT (sticker_file_id, tag) DO NOTHING;
