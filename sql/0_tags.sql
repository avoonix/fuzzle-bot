-- Category: Rating (99)
INSERT INTO tag (id, category, aliases) VALUES ('safe', 99, '["s", "rating:s", "rating:safe", "sfw"]');
INSERT INTO tag (id, category, aliases) VALUES ('questionable', 99, '["q", "rating:q", "rating:questionable"]');
INSERT INTO tag (id, category, aliases) VALUES ('explicit', 99, '["e", "rating:e", "rating:explicit"]');

-- Category: Meta (7)
INSERT INTO tag (id, category, aliases) VALUES ('meta_sticker', 7, '["moved_info", "additional_info", "information", "advertisement", "placeholder", "artist_signature", "contact_info", "creator_sticker", "author_sticker"]');
INSERT INTO tag (id, category, implications, aliases) VALUES ('attribution', 7, '["meta_sticker"]', '["creator", "maker", "stickers_by"]');
INSERT INTO tag (id, category, aliases) VALUES ('segmented_sticker', 7, '["split_sticker", "combined_sticker", "puzzle_sticker", "composite_sticker", "modular_sticker", "match-up_sticker"]');
INSERT INTO tag (id, category, aliases) VALUES ('irrelevant_content', 7, '["irrelevant_sticker", "trash_memes", "memes", "dank_memes", "cute_animals", "irl"]');

-- Category: Species (5)
INSERT INTO tag (id, category) VALUES ('diaper_creature', 5);

-- Category: Character (4)
INSERT INTO tag (id, category, aliases) VALUES ('ych_(character)', 4, '["ych", "you", "your_character_here", "placeholder_character", "blank_character"]');

-- Category: General (0)
INSERT INTO tag (id, category) VALUES ('yes', 0);
INSERT INTO tag (id, category, implications) VALUES ('thumbs_up', 0, '["yes"]');
INSERT INTO tag (id, category) VALUES ('no', 0);
INSERT INTO tag (id, category, implications) VALUES ('thumbs_down', 0, '["no"]');
INSERT INTO tag (id, category, implications) VALUES ('holding_heart', 0, '["holding_object"]');
INSERT INTO tag (id, category) VALUES ('segufix', 0);
INSERT INTO tag (id, category) VALUES ('hiding_behind_tail', 0);
INSERT INTO tag (id, category, aliases) VALUES ('unsure', 0, '["ambivalent", "neutral"]');
INSERT INTO tag (id, category, aliases) VALUES ('ill', 0, '["sick"]');
INSERT INTO tag (id, category, aliases) VALUES ('halo', 0, '["innocent"]');
INSERT INTO tag (id, category, aliases) VALUES ('yeet', 0, '["throw"]');
INSERT INTO tag (id, category, aliases) VALUES ('a', 0, '["aaaaaa", "aaaa", "aa", "aaa", "single_letter_a"]');
INSERT INTO tag (id, category, aliases) VALUES ('male', 0, '["m"]');
INSERT INTO tag (id, category, aliases) VALUES ('female', 0, '["f"]');
INSERT INTO tag (id, category, aliases) VALUES ('solo', 0, '["1", "alone"]');
INSERT INTO tag (id, category, aliases) VALUES ('duo', 0, '["2"]');
INSERT INTO tag (id, category, aliases) VALUES ('trio', 0, '["3"]');
INSERT INTO tag (id, category, aliases) VALUES ('group', 0, '["4", "5", "6", "7", "8", "9"]');
INSERT INTO tag (id, category, aliases) VALUES ('screaming', 0, '["loud"]');
INSERT INTO tag (id, category, aliases) VALUES ('maned_wolf', 0, '["leggy"]');

-- Category: Artist (1)
INSERT INTO tag (id, category, aliases, linked_channel_id, linked_user_id) VALUES ('niuka_folfsky', 1, '["niuka", "niu-ka"]', -1001160345567, 218075668);
INSERT INTO tag (id, category, aliases, linked_channel_id, linked_user_id) VALUES ('nowandlater', 1, '["nal", "nav", "cinnamonspots"]', -1001128919121, 123869615);
INSERT INTO tag (id, category, aliases, linked_channel_id, linked_user_id) VALUES ('stumblinbear', 1, '["king_seff"]', -1001493513255, 104504591);
INSERT INTO tag (id, category, aliases, linked_channel_id, linked_user_id) VALUES ('yuniwusky', 1, '["yuni"]', -1001101647690, 110037233);
INSERT INTO tag (id, category, aliases, linked_channel_id, linked_user_id) VALUES ('firelex', 1, '["alextheyellowthing", "alex"]', -1001395589417, 334417239);
INSERT INTO tag (id, category, linked_channel_id, linked_user_id) VALUES ('dlw', 1, -1002188616432, 29781141);
INSERT INTO tag (id, category, aliases, linked_channel_id, linked_user_id) VALUES ('rainyote', 1, '["mountaindewdrawer"]', -1001211019704, 166371683);
INSERT INTO tag (id, category, aliases, linked_user_id) VALUES ('rustledfluff', 1, '["russ"]', 531173348);
INSERT INTO tag (id, category) VALUES ('keavemind', 1);
INSERT INTO tag (id, category, linked_channel_id, linked_user_id) VALUES ('felisrandomis', 1, -1001004256968, 101768483);
INSERT INTO tag (id, category, linked_channel_id, linked_user_id) VALUES ('spookyfoxinc', 1, -1001275246621, 290830363);
INSERT INTO tag (id, category, linked_channel_id, linked_user_id) VALUES ('rbkfury', 1, -1001223421656, 767361635);
INSERT INTO tag (id, category, linked_channel_id, linked_user_id) VALUES ('pulex', 1, -1001135315393, 117178349);

-- TODO: more artists
