use std::collections::HashMap;

use crate::{
    bot::BotError,
    callback::CallbackData,
    database::UserStats,
    inline::{InlineQueryData, SetOperation},
    tags::{self, all_count_tags, all_rating_tags, character_count, rating, Characters},
};
use itertools::Itertools;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, LoginUrl};
use url::Url;

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

        let suggested_tags = suggested_tags
            .iter()
            .filter(|tag| {
                character_count(&(*tag).to_string()).is_none()
                    && tags::rating(&(*tag).to_string()).is_none()
            })
            .filter(|tag| !button_layout.iter().flatten().any(|button| &button == tag))
            .cloned()
            .collect::<Vec<String>>();

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

        InlineKeyboardMarkup::new(keyboard)
    }

    #[must_use]
    pub fn similarity(sticker_unique_id: &str) -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(vec![
            vec![InlineKeyboardButton::switch_inline_query_current_chat(
                "Similar color (no blacklist)",
                InlineQueryData::similar(sticker_unique_id, crate::inline::SimilarityAspect::Color)
            )],
            vec![InlineKeyboardButton::switch_inline_query_current_chat(
                "Similar shape (no blacklist)",
                InlineQueryData::similar(sticker_unique_id, crate::inline::SimilarityAspect::Shape)
            )],
        ])
    }

    #[must_use]
    pub fn make_blacklist_keyboard(current_blacklist: &[String]) -> InlineKeyboardMarkup {
        let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];

        keyboard.push(vec![InlineKeyboardButton::callback(
            "🔙 Settings",
            CallbackData::Settings,
        )]);

        for tag in current_blacklist {
            keyboard.push(vec![InlineKeyboardButton::callback(
                format!("Remove \"{tag}\""),
                CallbackData::RemoveBlacklistedTag(tag.to_string()),
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
                InlineQueryData::set_operation(set_name, vec![], SetOperation::Untag),
            ),
        ]];

        InlineKeyboardMarkup::new(keyboard)
    }

    #[must_use]
    pub fn make_main_keyboard() -> InlineKeyboardMarkup {
        let keyboard: Vec<Vec<InlineKeyboardButton>> = vec![
            vec![
                InlineKeyboardButton::callback("Help", CallbackData::Help),
                InlineKeyboardButton::callback("Settings", CallbackData::Settings),
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
            CallbackData::Help,
        )]])
    }

    #[must_use]
    pub fn make_help_keyboard() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new([
            [InlineKeyboardButton::callback(
                "🔙 Start",
                CallbackData::Start,
            )],
            [InlineKeyboardButton::callback(
                "Other Info",
                CallbackData::Info,
            )],
        ])
    }

    #[must_use]
    pub fn make_settings_keyboard() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new([
            [InlineKeyboardButton::callback(
                "🔙 Start",
                CallbackData::Start,
            )],
            [InlineKeyboardButton::callback(
                "Blacklist",
                CallbackData::Blacklist,
            )],
        ])
    }

    #[must_use]
    pub fn ui(domain_name: String) -> Result<InlineKeyboardMarkup, BotError> {
        Ok(InlineKeyboardMarkup::new([[InlineKeyboardButton::login(
            "Open",
            LoginUrl {
                url: Url::parse(&format!("https://{domain_name}/login"))?,
                forward_text: None,
                bot_username: None,
                request_write_access: None,
            },
        )]]))
    }

    #[must_use]
    pub fn user_stats(user_id: u64) -> Result<InlineKeyboardMarkup, BotError> {
        Ok(InlineKeyboardMarkup::new([[InlineKeyboardButton::url(
            "Show User",
            Url::parse(format!("tg://user?id={user_id}").as_str())?,
        )]]))
    }

    #[must_use]
    pub fn daily_report(
        tagging_stats: HashMap<Option<i64>, UserStats>,
    ) -> Result<InlineKeyboardMarkup, BotError> {
        Ok(InlineKeyboardMarkup::new(
            tagging_stats.into_iter().filter_map(
                |(
                    user_id,
                    UserStats {
                        added_tags,
                        removed_tags,
                    },
                )| {
                    user_id.map(|user_id| {
                        vec![InlineKeyboardButton::callback(
                            format!("{user_id}: +{added_tags} -{removed_tags}"),
                            CallbackData::user_info(user_id as u64),
                        )]
                    })
                },
            ),
        ))
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
