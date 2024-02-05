use crate::{
    callback::CallbackData,
    inline::{InlineQueryData, SetOperation},
    tags::{self, all_count_tags, all_rating_tags, character_count, rating, Characters},
};
use itertools::Itertools;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

pub struct Keyboard;

impl Keyboard {
    #[must_use]
    pub fn make_tag_keyboard(
        current_tags: &[String],
        sticker_unique_id: &str,
        suggested_tags: &[String],
        set_name: Option<String>,
    ) -> InlineKeyboardMarkup {
        let mut button_layout: Vec<Vec<String>> = vec![];
        // trio implies group; trio is more important than group
        let count = current_tags
            .iter()
            .filter_map(|tag| character_count(tag))
            .sorted()
            .next();
        let rating = current_tags.iter().find_map(|tag| rating(tag));

        let (_rating, count) = match (rating, count) {
            (Some(rating), Some(count)) => {
                button_layout.push(vec![rating.to_string(), count.to_string()]);
                (rating, count)
            }
            _ => {
                button_layout.push(count.map_or_else(
                    || {
                        all_count_tags()
                            .iter()
                            .map(std::string::ToString::to_string)
                            .collect()
                    },
                    |count| vec![count.to_string()],
                ));
                button_layout.push(rating.map_or_else(
                    || {
                        all_rating_tags()
                            .iter()
                            .map(std::string::ToString::to_string)
                            .collect()
                    },
                    |rating| vec![rating.to_string()],
                ));
                return InlineKeyboardMarkup::new(button_layout_to_keyboard_layout(
                    button_layout,
                    current_tags,
                    sticker_unique_id,
                ));
            }
        };

        match count {
            Characters::Zero => {}
            Characters::One => {
                button_layout.push(vec![
                    "male".to_string(),
                    "female".to_string(),
                    "ambiguous_gender".to_string(),
                ]);
            }
            _ => {
                button_layout.push(vec![
                    "male".to_string(),
                    "female".to_string(),
                    "ambiguous_gender".to_string(),
                ]);
                button_layout.push(vec!["male/male".to_string(), "male/female".to_string()]);
                button_layout.push(vec![
                    "ych_(character)".to_string(),
                    "female/female".to_string(),
                ]);
            }
        }

        let present_tags = current_tags
            .iter()
            .filter(|tag| {
                character_count(&(*tag).to_string()).is_none()
                    && tags::rating(&(*tag).to_string()).is_none()
            })
            .filter(|tag| !button_layout.iter().flatten().any(|button| &button == tag))
            .cloned()
            .collect::<Vec<String>>();

        for tags in present_tags.chunks(2) {
            button_layout.push(tags.iter().map(std::string::ToString::to_string).collect());
        }

        let mut suggested_tags = suggested_tags
            .iter()
            .filter(|tag| {
                character_count(&(*tag).to_string()).is_none()
                    && tags::rating(&(*tag).to_string()).is_none()
            })
            .filter(|tag| !button_layout.iter().flatten().any(|button| &button == tag))
            .cloned()
            .collect::<Vec<String>>();

        if suggested_tags.len() < 10 {
            // TODO: do this in suggest_tags
            let common_tags = [
                "young",
                "gore",
                "scat",
                "watersports",
                "diaper",
                "vore",
            ];
            for tag in &common_tags {
                if !suggested_tags.contains(&(*tag).to_string()) {
                    suggested_tags.push((*tag).to_string());
                }
            }
        }

        for tags in suggested_tags.chunks(2) {
            button_layout.push(tags.iter().map(std::string::ToString::to_string).collect());
        }

        let mut keyboard =
            button_layout_to_keyboard_layout(button_layout, current_tags, sticker_unique_id);
        keyboard.push(vec![
            InlineKeyboardButton::switch_inline_query_current_chat(
                "Add more tags to this sticker",
                InlineQueryData::empty_sticker_query(sticker_unique_id).to_string(),
            ),
        ]);

        // if let Some(set_name) = set_name {
        //     keyboard.push(vec![
        //         InlineKeyboardButton::switch_inline_query_current_chat(
        //             "Add tags to all stickers in this set",
        //             InlineQueryData::set_operation(set_name, vec![]).to_string(),
        //         ),
        //     ]);
        // } // TODO: maybe reenable?

        keyboard.push(vec![
            // InlineKeyboardButton::callback("Tag Set", CallbackData::
            // TODO: tag set button
        ]);

        InlineKeyboardMarkup::new(keyboard)
    }

    #[must_use]
    pub fn make_blacklist_keyboard(current_blacklist: &[String]) -> InlineKeyboardMarkup {
        let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];

        keyboard.push(vec![InlineKeyboardButton::callback(
            "🔙 Settings",
            CallbackData::Settings.to_string(),
        )]);

        for tag in current_blacklist {
            keyboard.push(vec![InlineKeyboardButton::callback(
                format!("Remove \"{tag}\""),
                CallbackData::RemoveBlacklistedTag(tag.to_string()).to_string(),
            )]);
        }

        keyboard.push(vec![
            InlineKeyboardButton::switch_inline_query_current_chat(
                "Add tag",
                InlineQueryData::blacklist_query(vec![]).to_string(),
            ),
        ]);

        InlineKeyboardMarkup::new(keyboard)
    }

    #[must_use]
    pub fn make_continuous_tag_keyboard() -> InlineKeyboardMarkup {
        let keyboard: Vec<Vec<InlineKeyboardButton>> = vec![
            vec![InlineKeyboardButton::switch_inline_query_current_chat(
                "Select tag",
                InlineQueryData::continuous_tag_mode(vec![], SetOperation::Tag).to_string(),
            )],
            vec![InlineKeyboardButton::switch_inline_query_current_chat(
                "Select tag (untag)", // TODO: better text
                InlineQueryData::continuous_tag_mode(vec![], SetOperation::Untag).to_string(),
            )],
        ];

        InlineKeyboardMarkup::new(keyboard)
    }

    #[must_use]
    pub fn make_set_keyboard(set_name: String) -> InlineKeyboardMarkup {
        // TODO: maybe check which tags exist across all stickers in the set
        // and show those (with a remove button)
        let keyboard: Vec<Vec<InlineKeyboardButton>> = vec![vec![
            InlineKeyboardButton::switch_inline_query_current_chat(
                "Add more tags to the set",
                InlineQueryData::set_operation(set_name.clone(), vec![], SetOperation::Tag)
                    .to_string(),
            ),
            InlineKeyboardButton::switch_inline_query_current_chat(
                "Remove tags from the set",
                InlineQueryData::set_operation(set_name, vec![], SetOperation::Untag).to_string(),
            ),
        ]];

        InlineKeyboardMarkup::new(keyboard)
    }

    #[must_use]
    pub fn make_main_keyboard() -> InlineKeyboardMarkup {
        let keyboard: Vec<Vec<InlineKeyboardButton>> = vec![
            vec![
                InlineKeyboardButton::callback("Help", CallbackData::Help.to_string()),
                InlineKeyboardButton::callback("Settings", CallbackData::Settings.to_string()),
            ],
            vec![InlineKeyboardButton::switch_inline_query(
                "Use me in a chat",
                "",
            )],
        ];

        InlineKeyboardMarkup::new(keyboard)
    }

    #[must_use]
    pub fn make_info_keyboard() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new([[InlineKeyboardButton::callback(
            "🔙 Help",
            CallbackData::Help.to_string(),
        )]])
    }

    #[must_use]
    pub fn make_help_keyboard() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new([
            [InlineKeyboardButton::callback(
                "🔙 Start",
                CallbackData::Start.to_string(),
            )],
            [InlineKeyboardButton::callback(
                "Other Info",
                CallbackData::Info.to_string(),
            )],
        ])
    }

    #[must_use]
    pub fn make_settings_keyboard() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new([
            [InlineKeyboardButton::callback(
                "🔙 Start",
                CallbackData::Start.to_string(),
            )],
            [InlineKeyboardButton::callback(
                "Blacklist",
                CallbackData::Blacklist.to_string(),
            )],
        ])
    }
}

fn button_layout_to_keyboard_layout(
    button_layout: Vec<Vec<String>>,
    current_tags: &[String],
    sticker_unique_id: &str,
) -> Vec<Vec<InlineKeyboardButton>> {
    let keyboard = button_layout
        .iter()
        .map(|row| {
            row.iter()
                .map(|tag| tag_to_button(tag, current_tags, sticker_unique_id))
                .collect()
        })
        .collect();
    keyboard
}

fn tag_to_button(
    tag: &str,
    current_tags: &[String],
    sticker_unique_id: &str,
) -> InlineKeyboardButton {
    let is_already_tagged = current_tags.contains(&tag.to_string());
    let callback_data = if is_already_tagged {
        CallbackData::untag_sticker(sticker_unique_id, tag.to_string())
    } else {
        CallbackData::tag_sticker(sticker_unique_id, tag.to_string())
    };
    let text = if is_already_tagged {
        format!("✅ {}", tag.to_owned())
    } else {
        tag.to_owned()
    };
    InlineKeyboardButton::callback(text, callback_data.to_string())
}
