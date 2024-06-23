use std::collections::HashMap;

use crate::{
    bot::{InternalError},
    callback::CallbackData,
    database::{UserSettings, UserStats},
    inline::{InlineQueryData, SetOperation},
    tags::{self, all_count_tags, all_rating_tags, character_count, rating, Characters}, util::Emoji,
};
use chrono::NaiveDateTime;
use itertools::Itertools;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, LoginUrl, UserId};
use url::Url;

pub struct Keyboard;

impl Keyboard {
    #[must_use]
    pub fn tagging(
        current_tags: &[String],
        sticker_unique_id: &str,
        suggested_tags: &[String],
        tagging_locked: bool,
        is_continuous_tag: bool,
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
                return InlineKeyboardMarkup::new(add_sticker_main_menu(
                    sticker_unique_id,
                    button_layout_to_keyboard_layout(
                        button_layout,
                        current_tags,
                        sticker_unique_id,
                    ),
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

        let mut keyboard = add_sticker_main_menu(
            sticker_unique_id,
            button_layout_to_keyboard_layout(button_layout, current_tags, sticker_unique_id),
        );
        keyboard.push(vec![
            InlineKeyboardButton::switch_inline_query_current_chat(
                "Add more tags to this sticker",
                InlineQueryData::empty_sticker_query(sticker_unique_id).to_string(),
            ),
        ]);

        let show_lock_buttons =
            tagging_locked || current_tags.contains(&"meta_sticker".to_string());

        if show_lock_buttons {
            keyboard.push(vec![if tagging_locked {
                InlineKeyboardButton::callback(
                    "🔒 Bulk Tagging Locked",
                    CallbackData::SetLock {
                        lock: false,
                        sticker_id: sticker_unique_id.to_string(),
                    },
                )
            } else {
                InlineKeyboardButton::callback(
                    "🔓 Bulk Tagging Unlocked",
                    CallbackData::SetLock {
                        lock: true,
                        sticker_id: sticker_unique_id.to_string(),
                    },
                )
            }]);
        }

        if is_continuous_tag {
keyboard.push(vec![InlineKeyboardButton::callback(
            "❌ Exit Continuous Tag Mode",
            CallbackData::ExitDialog,
        )]);
        }

        InlineKeyboardMarkup::new(keyboard)
    }

    // #[must_use]
    // pub fn similarity(sticker_unique_id: &str) -> InlineKeyboardMarkup {
    //     InlineKeyboardMarkup::new(vec![
    //     ])
    // }

    #[must_use]
    pub fn blacklist(current_blacklist: &[String]) -> InlineKeyboardMarkup {
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
    pub fn recommender(sticker_id: &str, similar: &[String], dissimilar: &[String], is_favorite: bool) -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(vec![
            vec![InlineKeyboardButton::callback(
            if similar.iter().any(|s| s == sticker_id) {
                "✅ More like this"
            } else {
                "More like this"
            },
            CallbackData::ToggleRecommendSticker{sticker_id: sticker_id.to_string(), positive: true},
        ), InlineKeyboardButton::callback(
            if dissimilar.iter().any(|d| d == sticker_id) {
                "✅ Less like this"
            } else {
                "Less like this"
            },
            CallbackData::ToggleRecommendSticker{sticker_id: sticker_id.to_string(), positive: false},
        )],
        vec![favorite_button(is_favorite, sticker_id)],
            vec![InlineKeyboardButton::switch_inline_query_current_chat(
                "🎨 Color",
                InlineQueryData::similar(sticker_id, crate::inline::SimilarityAspect::Color),
            ), InlineKeyboardButton::switch_inline_query_current_chat(
                "🦄 Similar",
                InlineQueryData::similar(sticker_id, crate::inline::SimilarityAspect::Embedding),
            ), InlineKeyboardButton::switch_inline_query_current_chat(
                format!("🪞 Sets"),
                InlineQueryData::overlapping_sets(sticker_id.to_string()),
            )],
            vec![
InlineKeyboardButton::switch_inline_query_current_chat(
                format!("🪄 Recommend"),
                InlineQueryData::recommendations(),
            )
            ]
        ])
    }

    #[must_use]
    pub fn removed_set(is_admin: bool, set_name: String) -> InlineKeyboardMarkup {
        if is_admin {
            InlineKeyboardMarkup::new([[InlineKeyboardButton::callback(
                "Unban Set",
                CallbackData::change_set_status(set_name, false),
            )]])
        } else {
            InlineKeyboardMarkup::new([[]])
        }
    }

    #[must_use]
    pub fn make_continuous_tag_keyboard(show_cancel: bool, add_tags: &[String], remove_tags: &[String]) -> InlineKeyboardMarkup {

        let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![
            vec![InlineKeyboardButton::switch_inline_query_current_chat(
                "Select tag",
                InlineQueryData::continuous_tag_mode(vec![], SetOperation::Tag).to_string(),
            )],
            vec![InlineKeyboardButton::switch_inline_query_current_chat(
                "Select tag (untag)", // TODO: better text
                InlineQueryData::continuous_tag_mode(vec![], SetOperation::Untag).to_string(),
            )],
        ];

        for tag in add_tags.into_iter().chain(remove_tags) {
            keyboard.push(vec![InlineKeyboardButton::callback(
                format!("Remove \"{tag}\" from the list"),
                CallbackData::RemoveContinuousTag(tag.to_string()),
            )]);
        }

        if show_cancel {
        keyboard.push(vec![InlineKeyboardButton::callback(
            "❌ Exit Continuous Tag Mode",
            CallbackData::ExitDialog,
        )]);
        }

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
    pub fn embedding() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new([[InlineKeyboardButton::switch_inline_query_current_chat(
            "Semantic search (⚠️ ignores blacklist)",
            InlineQueryData::embedding("stupid inside box".to_string()),
        )]])
    }

    #[must_use]
    pub fn make_main_keyboard() -> InlineKeyboardMarkup {
        let keyboard: Vec<Vec<InlineKeyboardButton>> = vec![
            vec![
                InlineKeyboardButton::callback("Help", CallbackData::Help),
                InlineKeyboardButton::callback("Settings", CallbackData::Settings),
            ],
            vec![InlineKeyboardButton::callback(
                "What else can you do?",
                CallbackData::FeatureOverview,
            )],
            vec![InlineKeyboardButton::switch_inline_query(
                "Use me in a chat",
                "",
            )],
        ];

        InlineKeyboardMarkup::new(keyboard)
    }

    #[must_use]
    pub fn emoji_article(emoji: Emoji) -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(vec![
            vec![InlineKeyboardButton::switch_inline_query_current_chat(
                format!("List stickers (ignores blacklist)"),
                InlineQueryData::search_emoji(vec![], vec![emoji]),
            )],
        ])
    }

    #[must_use]
    pub fn general_stats() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(vec![
            vec![
                InlineKeyboardButton::callback("⬅️ Latest Sets", CallbackData::LatestSets),
                InlineKeyboardButton::callback("Personal Stats ➡️", CallbackData::PersonalStats),
            ],
            vec![InlineKeyboardButton::switch_inline_query_current_chat(
                format!("Stickers with most duplicates"),
                InlineQueryData::most_duplicated_stickers(),
            )],
            vec![InlineKeyboardButton::switch_inline_query_current_chat(
                format!("Most popular emojis"),
                InlineQueryData::most_used_emojis(),
            )],
        ])
    }

    #[must_use]
    pub fn personal_stats() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new([[
            InlineKeyboardButton::callback("⬅️ General Stats", CallbackData::GeneralStats),
            InlineKeyboardButton::callback("Popular Tags ➡️", CallbackData::PopularTags),
        ]])
    }

    #[must_use]
    pub fn popular_tags() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new([[
            InlineKeyboardButton::callback("⬅️ Personal Stats", CallbackData::PersonalStats),
            InlineKeyboardButton::callback("Latest Sets ➡️", CallbackData::LatestSets),
        ]])
    }

    #[must_use]
    pub fn latest_sets() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new([[
            InlineKeyboardButton::callback("⬅️ Popular Tags", CallbackData::PopularTags),
            InlineKeyboardButton::callback("General Stats ➡️", CallbackData::GeneralStats),
        ]])
    }

    #[must_use]
    pub fn dialog_exit() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new([[InlineKeyboardButton::callback(
            "❌ Exit Mode",
            CallbackData::ExitDialog,
        )]])
    }

    #[must_use]
    pub fn info() -> InlineKeyboardMarkup {
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
    pub fn make_settings_keyboard(settings: &UserSettings) -> InlineKeyboardMarkup {
        let order = match settings.order() {
            crate::database::StickerOrder::LatestFirst => InlineKeyboardButton::callback(
                "🔀 Change Ordering",
                CallbackData::SetOrder(crate::database::StickerOrder::Random),
            ),
            crate::database::StickerOrder::Random => InlineKeyboardButton::callback(
                "🆕 Change Ordering",
                CallbackData::SetOrder(crate::database::StickerOrder::LatestFirst),
            ),
        };

        InlineKeyboardMarkup::new([
            [InlineKeyboardButton::callback(
                "🔙 Start",
                CallbackData::Start,
            )],
            [order],
            [InlineKeyboardButton::callback(
                "Blacklist",
                CallbackData::Blacklist,
            )],
        ])
    }

    #[must_use]
    pub fn ui(domain_name: String) -> Result<InlineKeyboardMarkup, InternalError> {
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
    pub fn user_stats(user_id: u64) -> Result<InlineKeyboardMarkup, InternalError> {
        Ok(InlineKeyboardMarkup::new([[InlineKeyboardButton::url(
            "Show User",
            Url::parse(format!("tg://user?id={user_id}").as_str())?,
        )]]))
    }

    #[must_use]
    pub fn daily_report(
        tagging_stats: HashMap<Option<i64>, UserStats>,
    ) -> Result<InlineKeyboardMarkup, InternalError> {
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

    #[must_use]
    pub fn new_user(user_id: UserId) -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new([[user_button(user_id)]])
    }

    #[must_use]
    pub fn merge(
        sticker_id_a: &str,
        sticker_id_b: &str,
        set_id_a: &str,
        set_id_b: &str,
    ) -> Result<InlineKeyboardMarkup, InternalError> {
        Ok(InlineKeyboardMarkup::new(vec![
            if set_id_a == set_id_b {
                vec![set_button(set_id_a)?]
            } else {
                vec![set_button(set_id_a)?, set_button(set_id_b)?]
            },
            vec![
                InlineKeyboardButton::callback(
                    "👍 Merge",
                    CallbackData::merge(sticker_id_a, sticker_id_b, true),
                ),
                InlineKeyboardButton::callback(
                    "👎 Don't Merge",
                    CallbackData::merge(sticker_id_a, sticker_id_b, false),
                ),
            ],
        ]))
    }

    #[must_use]
    pub fn merge_done(set_id_a: &str, set_id_b: &str) -> Result<InlineKeyboardMarkup, InternalError> {
        Ok(InlineKeyboardMarkup::new(vec![if set_id_a == set_id_b {
            vec![set_button(set_id_a)?]
        } else {
            vec![set_button(set_id_a)?, set_button(set_id_b)?]
        }]))
    }

    #[must_use]
    pub fn new_set(
        submitted_by: Option<UserId>,
        set_name: &str,
    ) -> Result<InlineKeyboardMarkup, InternalError> {
        Ok(InlineKeyboardMarkup::new(vec![
            vec![set_button(set_name)?],
            vec![InlineKeyboardButton::callback(
                "Delete/Ban Set",
                CallbackData::change_set_status(set_name, true),
            )],
            submitted_by.map_or_else(Vec::new, |submitted_by| vec![user_button(submitted_by)]),
        ]))
    }

    #[must_use]
    pub fn sticker_set_page(
        sticker_id: &str,
        set_id: &str,
        created_at: NaiveDateTime,
    ) -> InlineKeyboardMarkup {
        let now = chrono::Utc::now().naive_utc();

        InlineKeyboardMarkup::new([
            [InlineKeyboardButton::callback(
                "🔙 Set",
                CallbackData::Sticker {
                    unique_id: sticker_id.to_string(),
                    operation: None,
                },
            )],
            [InlineKeyboardButton::callback(
                format!(
                    "🗓️ Set known since {} ({} days)",
                    created_at.format("%Y-%m-%d"),
                    (now - created_at).num_days()
                ),
                CallbackData::NoAction,
            )],
            [InlineKeyboardButton::switch_inline_query_current_chat(
                format!("🪞 Duplicates from other sets"),
                InlineQueryData::overlapping_sets(sticker_id.to_string()),
            )],
            [InlineKeyboardButton::switch_inline_query_current_chat(
                format!("🏷️ All tags"),
                InlineQueryData::all_set_tags(sticker_id.to_string()),
            )],
            [InlineKeyboardButton::switch_inline_query_current_chat(
                format!("➕ Add tags to all stickers in the set \"{set_id}\""),
                InlineQueryData::set_operation(set_id, vec![], SetOperation::Tag),
            )],
            [InlineKeyboardButton::switch_inline_query_current_chat(
                format!("➖ Remove tags from all stickers in the set \"{set_id}\""),
                InlineQueryData::set_operation(set_id, vec![], SetOperation::Untag),
            )],
        ])
    }

    #[must_use]
    pub fn sticker_explore_page(
        sticker_id: &str,
        set_count: usize,
        created_at: NaiveDateTime,
        is_favorite: bool,
    ) -> InlineKeyboardMarkup {
        let now = chrono::Utc::now().naive_utc();
        let set_text = if set_count == 1 {
            "set contains"
        } else {
            "sets contain"
        };

        InlineKeyboardMarkup::new([
            [InlineKeyboardButton::callback(
                "🔙 Sticker",
                CallbackData::Sticker {
                    unique_id: sticker_id.to_string(),
                    operation: None,
                },
            )],
            [favorite_button(is_favorite, sticker_id)],
            [InlineKeyboardButton::callback(
                format!(
                    "🗓️ Sticker known since {} ({} days)",
                    created_at.format("%Y-%m-%d"),
                    (now - created_at).num_days()
                ),
                CallbackData::NoAction,
            )],
            [InlineKeyboardButton::callback(
                format!("📥 Download file"),
                CallbackData::DownloadSticker {
                    sticker_id: sticker_id.to_string(),
                },
            )],
            [InlineKeyboardButton::switch_inline_query_current_chat(
                "🎨 Similarly colored stickers (⚠️ ignores blacklist)",
                InlineQueryData::similar(sticker_id, crate::inline::SimilarityAspect::Color),
            )],
            // vec![InlineKeyboardButton::switch_inline_query_current_chat(
            //     "Similar shape (Warning: ignores blacklist)",
            //     InlineQueryData::similar(sticker_unique_id, crate::inline::SimilarityAspect::Shape),
            // )],
            [InlineKeyboardButton::switch_inline_query_current_chat(
                "🦄 Similar stickers (⚠️ ignores blacklist)",
                InlineQueryData::similar(sticker_id, crate::inline::SimilarityAspect::Embedding),
            )],
            [InlineKeyboardButton::switch_inline_query_current_chat(
                format!("🗂️ {set_count} {set_text} this sticker"),
                InlineQueryData::sets(sticker_id.to_string()),
            )],
            // [InlineKeyboardButton::callback(
            //     "Other Info",
            //     CallbackData::Info,
            // )],
        ])
    }
}

fn add_sticker_main_menu(
    sticker_id: &str,
    other_buttons: Vec<Vec<InlineKeyboardButton>>,
) -> Vec<Vec<InlineKeyboardButton>> {
    vec![vec![
        InlineKeyboardButton::callback(
            "️🗂️ Set",
            CallbackData::StickerSetPage {
                sticker_id: sticker_id.to_string(),
            },
        ),
        InlineKeyboardButton::callback(
            "✨️ Sticker",
            CallbackData::StickerExplorePage {
                sticker_id: sticker_id.to_string(),
            },
        ),
    ]]
    .into_iter()
    .chain(other_buttons)
    .collect_vec()
}

fn user_button(user_id: UserId) -> InlineKeyboardButton {
    InlineKeyboardButton::url(
        format!("Show User {user_id}"),
        Url::parse(format!("tg://user?id={}", user_id.0).as_str()).expect("url to be valid"),
    )
}

fn set_button(set_id: &str) -> Result<InlineKeyboardButton, InternalError> {
    Ok(InlineKeyboardButton::url(
        "Open Set",
        Url::parse(format!("https://t.me/addstickers/{set_id}").as_str())?,
    ))
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

fn favorite_button(is_favorite: bool, sticker_id: &str) -> InlineKeyboardButton {
if is_favorite {
                InlineKeyboardButton::callback(
                    "⭐ Favorite",
                    CallbackData::FavoriteSticker {
                        sticker_id: sticker_id.to_string(),
                        operation: crate::callback::FavoriteAction::Unfavorite,
                    },
                )
            } else {
                InlineKeyboardButton::callback(
                    "⚫ Mark as favorite",
                    CallbackData::FavoriteSticker {
                        sticker_id: sticker_id.to_string(),
                        operation: crate::callback::FavoriteAction::Favorite,
                    },
                )
            }
}