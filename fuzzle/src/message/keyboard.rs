use std::{collections::HashMap, sync::Arc};

use crate::{
    background_tasks::{GetCategory, TagManagerWorker},
    bot::InternalError,
    callback::CallbackData,
    database::{
        ModerationTaskStatus, Sticker, StickerChange, Tag, TagCreator, UserSettings, UserStats,
        UserStickerStat,
    },
    inline::{InlineQueryData, SetOperation, TagKind},
    tags::{self, all_count_tags, all_rating_tags, character_count, rating, Category, Characters},
    util::{format_relative_time, Emoji},
};
use chrono::NaiveDateTime;
use futures::future::try_join_all;
use itertools::Itertools;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, LoginUrl, UserId};
use url::Url;

use super::PrivacyPolicy;

pub struct Keyboard;

impl Keyboard {
    #[must_use]
    pub async fn tagging(
        current_tags: &[String],
        sticker_unique_id: &str,
        suggested_tags: &[String],
        tagging_locked: bool,
        is_continuous_tag: bool,
        tag_manager: TagManagerWorker,
    ) -> Result<InlineKeyboardMarkup, InternalError> {
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
                // button_layout.push(vec![rating.to_string(), count.to_string()]);
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
                return Ok(InlineKeyboardMarkup::new(add_sticker_main_menu(
                    sticker_unique_id,
                    button_layout_to_keyboard_layout(
                        button_layout,
                        current_tags,
                        sticker_unique_id,
                        tag_manager,
                    )
                    .await?,
                )));
            }
        };

        let present_tags = current_tags
            .iter()
            // .filter(|tag| {
            //     character_count(&(*tag).to_string()).is_none()
            //         && tags::rating(&(*tag).to_string()).is_none()
            // })
            .filter(|tag| !button_layout.iter().flatten().any(|button| &button == tag))
            .cloned()
            .collect::<Vec<String>>();

        'outer: for tag in present_tags.into_iter().sorted_by_key(|s| s.len()).rev() {
            for last in button_layout.iter_mut() {
                if can_insert_tag_in_column(last.as_slice(), &tag, 3) {
                    last.push(tag);
                    continue 'outer;
                }
            }

            button_layout.push(vec![tag]);
        }

        button_layout.push(vec![]);

        let suggested_tags = suggested_tags
            .iter()
            .filter(|tag| {
                // TODO: instead of filtering this, allow these suggestions + remove incompatible tags if clicked (eg safe is present, user clicks explicit, -> safe is removed)
                character_count(&(*tag).to_string()).is_none()
                    && tags::rating(&(*tag).to_string()).is_none()
            })
            .filter(|tag| !button_layout.iter().flatten().any(|button| &button == tag))
            .cloned()
            .collect::<Vec<String>>();

        // for tags in &suggested_tags.into_iter().chunks(2) {
        //     button_layout.push(tags.collect());
        // }
        for tag in suggested_tags.into_iter() {
            if let Some(last) = button_layout.last_mut() {
                if can_insert_tag_in_column(last.as_slice(), &tag, 0) {
                    last.push(tag);
                    continue;
                }
            }

            button_layout.push(vec![tag]);
        }

        let mut keyboard = add_sticker_main_menu(
            sticker_unique_id,
            button_layout_to_keyboard_layout(
                button_layout,
                current_tags,
                sticker_unique_id,
                tag_manager,
            )
            .await?,
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

        Ok(InlineKeyboardMarkup::new(keyboard))
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
    pub fn tag_creator_initial() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(vec![vec![
            InlineKeyboardButton::switch_inline_query_current_chat(
                format!("Name input"),
                InlineQueryData::tag_creator("".to_string(), TagKind::Main),
            ),
        ]])
    }

    fn moderation_task_common(
        current_status: ModerationTaskStatus,
        creator_id: i64,
        task_id: i64,
    ) -> Vec<InlineKeyboardButton> {
        let status_list = [
            (ModerationTaskStatus::Pending, "Pending"),
            (ModerationTaskStatus::Completed, "Completed"),
            (ModerationTaskStatus::Cancelled, "Cancelled"),
        ];
        status_list
            .into_iter()
            .map(|(status, text)| {
                InlineKeyboardButton::callback(
                    if current_status == status {
                        format!("[{text}]")
                    } else {
                        text.to_string()
                    },
                    CallbackData::ChangeModerationTaskStatus { status, task_id },
                )
            })
            .chain(vec![user_button(UserId(creator_id as u64))])
            .collect_vec()
    }

    #[must_use]
    pub fn create_tag_task(
        status: ModerationTaskStatus,
        creator_id: i64,
        task_id: i64,
        user_username: Option<String>,
        channel_username: Option<String>,
    ) -> InlineKeyboardMarkup {
        // todo! add useful buttons to approve (+undo if already exists)
        InlineKeyboardMarkup::new(
            vec![
                vec![Self::moderation_task_common(status, creator_id, task_id)],
                if let Some(user_username) = user_username {
                    vec![vec![InlineKeyboardButton::url(
                        format!("@{user_username}"),
                        Url::parse(format!("https://t.me/{}", user_username).as_str())
                            .expect("url to be valid"),
                    )]]
                } else {
                    vec![]
                },
                if let Some(channel_username) = channel_username {
                    vec![vec![InlineKeyboardButton::url(
                        format!("@{channel_username}"),
                        Url::parse(format!("https://t.me/{}", channel_username).as_str())
                            .expect("url to be valid"),
                    )]]
                } else {
                    vec![]
                },
                vec![vec![
                    InlineKeyboardButton::callback(
                        "Approve Tag",
                        CallbackData::TagListAction {
                            moderation_task_id: task_id,
                            action: crate::callback::TagListAction::Add,
                        },
                    ),
                    InlineKeyboardButton::callback(
                        "Delete Tag",
                        CallbackData::TagListAction {
                            moderation_task_id: task_id,
                            action: crate::callback::TagListAction::Remove,
                        },
                    ),
                ]],
            ]
            .concat(),
        )
    }

    #[must_use]
    pub fn report_sticker_set_task(
        status: ModerationTaskStatus,
        creator_id: i64,
        task_id: i64,
        set_name: &str,
        banned: bool,
    ) -> Result<InlineKeyboardMarkup, InternalError> {
        Ok(InlineKeyboardMarkup::new(vec![
            Self::moderation_task_common(status, creator_id, task_id),
            vec![
                set_button(set_name)?,
                if banned {
                    InlineKeyboardButton::callback(
                        format!("Unban {}", set_name),
                        CallbackData::change_set_status(set_name, false, task_id),
                    )
                } else {
                    InlineKeyboardButton::callback(
                        format!("Ban {}", set_name),
                        CallbackData::change_set_status(set_name, true, task_id),
                    )
                },
            ],
        ]))
    }

    #[must_use]
    pub fn review_new_sets_task(
        status: ModerationTaskStatus,
        creator_id: i64,
        task_id: i64,
        added_sets: &[String],
        indexed_sets: &[String],
    ) -> Result<InlineKeyboardMarkup, InternalError> {
        let mut markup = InlineKeyboardMarkup::new(vec![Self::moderation_task_common(
            status, creator_id, task_id,
        )]);

        for set_name in added_sets {
            markup = markup.append_row(vec![
                set_button(set_name)?,
                if indexed_sets.contains(set_name) {
                    InlineKeyboardButton::callback(
                        format!("Ban {}", set_name),
                        CallbackData::change_set_status(set_name, true, task_id),
                    )
                } else {
                    InlineKeyboardButton::callback(
                        format!("Unban {}", set_name),
                        CallbackData::change_set_status(set_name, false, task_id),
                    )
                },
            ]);
        }

        Ok(markup)
    }

    #[must_use]
    pub fn tag_creator(state: &TagCreator) -> InlineKeyboardMarkup {
        let mut markup = InlineKeyboardMarkup::new(vec![vec![
            InlineKeyboardButton::switch_inline_query_current_chat(
                format!("Change tag name"),
                InlineQueryData::tag_creator(state.tag_id.to_string(), TagKind::Main),
            ),
            InlineKeyboardButton::switch_inline_query_current_chat(
                format!("Add alias"),
                InlineQueryData::tag_creator("".to_string(), TagKind::Alias),
            ),
        ]]);
        if let Some(ref channel) = state.linked_channel {
            markup = markup.append_row(vec![InlineKeyboardButton::callback(
                format!("Remove linked channel {}", channel), // TODO: get username from db
                CallbackData::RemoveLinkedChannel,
            )]);
        }
        if let Some(ref user) = state.linked_user {
            markup = markup.append_row(vec![InlineKeyboardButton::callback(
                format!("Remove linked user {}", user), // TODO: get username from db
                CallbackData::RemoveLinkedUser,
            )]);
        } else {
            markup = markup.append_row(vec![InlineKeyboardButton::callback(
                "Link yourself",
                CallbackData::LinkSelf,
            )]);
        }
        for alias in state.aliases.iter() {
            markup = markup.append_row(vec![InlineKeyboardButton::callback(
                format!("Remove alias {}", &alias),
                CallbackData::RemoveAlias(alias.to_string()),
            )]);
        }
        markup = markup.append_row([Category::Artist, Category::Character].into_iter().map(
            |category| {
                if state.category == Some(category) {
                    InlineKeyboardButton::callback(
                        format!("✅ {}", category.to_human_name()),
                        CallbackData::SetCategory(None),
                    )
                } else {
                    InlineKeyboardButton::callback(
                        format!("{}", category.to_human_name()),
                        CallbackData::SetCategory(Some(category)),
                    )
                }
            },
        ));
        markup = markup.append_row(vec![InlineKeyboardButton::callback(
            format!("Create tag {}", &state.tag_id),
            CallbackData::CreateTag,
        )]);

        markup
    }

    #[must_use]
    pub fn tag_creator_sticker(state: &TagCreator, sticker: &Sticker) -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(vec![vec![
            if state.example_sticker_id.contains(&sticker.id) {
                InlineKeyboardButton::callback(
                    "✅ Use as example",
                    CallbackData::ToggleExampleSticker {
                        sticker_id: sticker.id.clone(),
                    },
                )
            } else {
                InlineKeyboardButton::callback(
                    "Use as example",
                    CallbackData::ToggleExampleSticker {
                        sticker_id: sticker.id.clone(),
                    },
                )
            },
        ]])
    }

    #[must_use]
    pub fn recommender(
        sticker_id: &str,
        similar: &[String],
        dissimilar: &[String],
        is_favorite: bool,
    ) -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(vec![
            vec![
                InlineKeyboardButton::callback(
                    if similar.iter().any(|s| s == sticker_id) {
                        "✅ More like this"
                    } else {
                        "More like this"
                    },
                    CallbackData::ToggleRecommendSticker {
                        sticker_id: sticker_id.to_string(),
                        positive: true,
                    },
                ),
                InlineKeyboardButton::callback(
                    if dissimilar.iter().any(|d| d == sticker_id) {
                        "✅ Less like this"
                    } else {
                        "Less like this"
                    },
                    CallbackData::ToggleRecommendSticker {
                        sticker_id: sticker_id.to_string(),
                        positive: false,
                    },
                ),
            ],
            vec![
                favorite_button(is_favorite, sticker_id),
                InlineKeyboardButton::switch_inline_query_current_chat(
                    "🔧 Add to your set",
                    InlineQueryData::add_to_user_set(sticker_id.to_string()),
                ),
            ],
            vec![
                InlineKeyboardButton::switch_inline_query_current_chat(
                    "🎨 Color",
                    InlineQueryData::similar(sticker_id, crate::inline::SimilarityAspect::Color),
                ),
                InlineKeyboardButton::switch_inline_query_current_chat(
                    "🦄 Similar",
                    InlineQueryData::similar(
                        sticker_id,
                        crate::inline::SimilarityAspect::Embedding,
                    ),
                ),
                InlineKeyboardButton::switch_inline_query_current_chat(
                    format!("🪞 Sets"),
                    InlineQueryData::overlapping_sets(sticker_id.to_string()),
                ),
            ],
            vec![InlineKeyboardButton::switch_inline_query_current_chat(
                format!("🪄 Recommend"),
                InlineQueryData::recommendations(),
            )],
        ])
    }

    #[must_use]
    pub fn make_continuous_tag_keyboard(
        show_cancel: bool,
        add_tags: &[String],
        remove_tags: &[String],
    ) -> InlineKeyboardMarkup {
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
    pub fn emoji_article(emoji: &Emoji) -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(vec![vec![
            InlineKeyboardButton::switch_inline_query_current_chat(
                format!("List stickers (ignores blacklist)"),
                InlineQueryData::search_emoji(vec![], vec![emoji.clone()]),
            ),
        ]])
    }

    #[must_use]
    pub fn general_stats() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(vec![
            stat_tabs(StatTab::General),
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
    pub fn personal_stats(user_id: i64) -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(vec![
            stat_tabs(StatTab::Personal),
            vec![InlineKeyboardButton::switch_inline_query_current_chat(
                "My sets",
                InlineQueryData::SetsByUserId { user_id: user_id },
            )],
        ])
    }

    #[must_use]
    pub fn popular_tags() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new([stat_tabs(StatTab::Popular)])
    }

    #[must_use]
    pub fn latest_sets() -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new([stat_tabs(StatTab::LatestSets)])
    }

    #[must_use]
    pub fn latest_stickers(changes: Vec<StickerChange>) -> InlineKeyboardMarkup {
        let mut markup = InlineKeyboardMarkup::new(vec![stat_tabs(StatTab::LatestStickers)]);
        for change in changes {
            markup = markup.append_row(vec![
                InlineKeyboardButton::switch_inline_query_current_chat(
                    format!("{}", change.sticker_set_id),
                    InlineQueryData::set_stickers_by_date(change.sticker_id), // TODO: change placeholder
                ),
            ]);
        }

        markup
    }

    #[must_use]
    pub fn general_user_stats(stats: Vec<UserStickerStat>) -> InlineKeyboardMarkup {
        let mut markup = InlineKeyboardMarkup::new(vec![stat_tabs(StatTab::User)]);
        for stat in stats.chunks(2) {
            markup = markup.append_row(stat.into_iter().map(|stat| {
                let username = stat.username.clone().map_or_else(
                    || {
                        stat.linked_tag.clone().map_or_else(
                            || format!("Unknown User"),
                            |linked_tag| format!("~{}", linked_tag),
                        )
                    },
                    |username| format!("@{username}"),
                );
                InlineKeyboardButton::switch_inline_query_current_chat(
                    format!("{username} ({} Sets)", stat.set_count), // TODO: some users are known - display those names?
                    InlineQueryData::SetsByUserId {
                        user_id: stat.user_id,
                    },
                )
            }));
        }

        markup.append_row(vec![
            InlineKeyboardButton::switch_inline_query_current_chat(
                "Show More",
                InlineQueryData::TopOwners,
            ),
        ])
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
        let mut keyboard = vec![vec![InlineKeyboardButton::callback(
            "🔙 Start",
            CallbackData::Start,
        )]];
        keyboard.extend(help_tabs(HelpTab::Other));
        InlineKeyboardMarkup::new(keyboard)
    }

    #[must_use]
    pub fn make_help_keyboard() -> InlineKeyboardMarkup {
        let mut keyboard = vec![vec![InlineKeyboardButton::callback(
            "🔙 Start",
            CallbackData::Start,
        )]];
        keyboard.extend(help_tabs(HelpTab::Commands));
        InlineKeyboardMarkup::new(keyboard)
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
    pub fn merge_done(
        set_id_a: &str,
        set_id_b: &str,
    ) -> Result<InlineKeyboardMarkup, InternalError> {
        Ok(InlineKeyboardMarkup::new(vec![if set_id_a == set_id_b {
            vec![set_button(set_id_a)?]
        } else {
            vec![set_button(set_id_a)?, set_button(set_id_b)?]
        }]))
    }

    #[must_use]
    pub fn sticker_set_page(
        sticker_id: &str,
        set_id: &str,
        created_at: NaiveDateTime,
    ) -> InlineKeyboardMarkup {
        let now = chrono::Utc::now().naive_utc();

        InlineKeyboardMarkup::new(vec![
            sticker_tabs(StickerTab::Set, sticker_id),
            vec![
                InlineKeyboardButton::callback(
                    format!("🗓️ Set added {}", format_relative_time(created_at)),
                    CallbackData::NoAction,
                ),
                InlineKeyboardButton::switch_inline_query_current_chat(
                    format!("🚩 Report Set"),
                    InlineQueryData::ReportSet {
                        set_id: set_id.to_string(),
                    },
                ),
            ],
            vec![InlineKeyboardButton::switch_inline_query_current_chat(
                format!("🪞 Set overlaps"),
                InlineQueryData::overlapping_sets(sticker_id.to_string()),
            )],
            vec![
                InlineKeyboardButton::switch_inline_query_current_chat(
                    format!("🏷️ All tags"),
                    InlineQueryData::all_set_tags(sticker_id.to_string()),
                ),
                InlineKeyboardButton::switch_inline_query_current_chat(
                    format!("🗓️ All stickers by date added"),
                    InlineQueryData::set_stickers_by_date(sticker_id.to_string()),
                ),
            ],
            vec![InlineKeyboardButton::switch_inline_query_current_chat(
                format!("➕ Add tags to all stickers in the set \"{set_id}\""),
                InlineQueryData::set_operation(set_id, vec![], SetOperation::Tag),
            )],
            vec![InlineKeyboardButton::switch_inline_query_current_chat(
                format!("➖ Remove tags from all stickers in the set \"{set_id}\""),
                InlineQueryData::set_operation(set_id, vec![], SetOperation::Untag),
            )],
        ])
    }

    #[must_use]
    fn owner(
        user_id: i64,
        set_count: i64,
        owner_username: Option<String>,
        owner_tags: Vec<Tag>,
        channel_usernames: Vec<(i64, String)>,
    ) -> Vec<Vec<InlineKeyboardButton>> {
        let linked_tags = owner_tags
            .into_iter()
            .map(|tag| {
                let tag_search = InlineKeyboardButton::switch_inline_query_current_chat(
                    format!("{} {}", tag.category.to_emoji(), &tag.id),
                    InlineQueryData::search(vec![tag.id]),
                );
                let linked_channel = tag
                    .linked_channel_id
                    .map(|channel_id| {
                        channel_usernames
                            .iter()
                            .find_map(|(id, username)| (*id == channel_id).then(|| username))
                    })
                    .flatten();

                if let Some(linked_channel) = linked_channel {
                    vec![
                        tag_search,
                        InlineKeyboardButton::url(
                            format!("@{linked_channel}"),
                            Url::parse(format!("https://t.me/{}", linked_channel).as_str())
                                .expect("url to be valid"),
                        ),
                    ]
                } else {
                    vec![tag_search]
                }
            })
            .collect_vec();

        let open_owner_button = if let Some(owner_username) = owner_username {
            vec![
                InlineKeyboardButton::url(
                    format!("@{owner_username}"),
                    Url::parse(format!("https://t.me/{}", owner_username).as_str())
                        .expect("url to be valid"),
                ),
                InlineKeyboardButton::callback(
                    "🏷️ Create Tag",
                    CallbackData::CreateTagForUser { user_id },
                ),
            ]
        } else {
            vec![
                InlineKeyboardButton::url(
                    format!("Show user if known (Android)"),
                    Url::parse(format!("tg://openmessage?user_id={}", user_id).as_str())
                        .expect("url to be valid"),
                ),
                InlineKeyboardButton::url(
                    format!("Show user if known (iOS)"),
                    Url::parse(format!("https://t.me/@id{}", user_id).as_str())
                        .expect("url to be valid"),
                ),
            ]
        };
        vec![
            vec![
                vec![InlineKeyboardButton::switch_inline_query_current_chat(
                    format!("{} sets owned by this user", set_count), // TODO: some users are known - display those names?
                    InlineQueryData::SetsByUserId { user_id: user_id },
                )],
                open_owner_button,
            ],
            linked_tags,
        ]
        .concat()
    }

    #[must_use]
    pub fn owner_page(
        sticker_id: &str,
        user_id: i64,
        set_count: i64,
        owner_username: Option<String>,
        owner_tags: Vec<Tag>,
        channel_usernames: Vec<(i64, String)>,
    ) -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(
            vec![
                vec![sticker_tabs(StickerTab::Owner, sticker_id)],
                Self::owner(
                    user_id,
                    set_count,
                    owner_username,
                    owner_tags,
                    channel_usernames,
                ),
            ]
            .concat(),
        )
    }

    #[must_use]
    pub fn owner_standalone(
        user_id: i64,
        set_count: i64,
        owner_username: Option<String>,
        owner_tags: Vec<Tag>,
        channel_usernames: Vec<(i64, String)>,
    ) -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(Self::owner(
            user_id,
            set_count,
            owner_username,
            owner_tags,
            channel_usernames,
        ))
    }

    #[must_use]
    pub fn sticker_explore_page(
        sticker_id: &str,
        set_count: usize,
        created_at: NaiveDateTime,
        is_favorite: bool,
        emoji: Option<Emoji>,
    ) -> InlineKeyboardMarkup {
        let now = chrono::Utc::now().naive_utc();
        let set_text = if set_count == 1 {
            "set contains"
        } else {
            "sets contain"
        };

        InlineKeyboardMarkup::new(
            vec![
                vec![
                    sticker_tabs(StickerTab::Sticker, sticker_id),
                    vec![InlineKeyboardButton::callback(
                        format!("🗓️ Sticker added {}", format_relative_time(created_at)),
                        CallbackData::NoAction,
                    )],
                    vec![
                        favorite_button(is_favorite, sticker_id),
                        InlineKeyboardButton::callback(
                            format!("📥 Download file"),
                            CallbackData::DownloadSticker {
                                sticker_id: sticker_id.to_string(),
                            },
                        ),
                    ],
                ],
                if let Some(emoji) = emoji {
                    vec![vec![
                        InlineKeyboardButton::switch_inline_query_current_chat(
                            format!(
                                "{} Stickers with this emoji",
                                emoji.to_string_with_variant()
                            ),
                            InlineQueryData::search_emoji(vec![], vec![emoji]),
                        ),
                    ]]
                } else {
                    vec![]
                },
                vec![
                    vec![InlineKeyboardButton::switch_inline_query_current_chat(
                        "🎨 Similarly colored stickers (⚠️ ignores blacklist)",
                        InlineQueryData::similar(
                            sticker_id,
                            crate::inline::SimilarityAspect::Color,
                        ),
                    )],
                    // vec![InlineKeyboardButton::switch_inline_query_current_chat(
                    //     "Similar shape (Warning: ignores blacklist)",
                    //     InlineQueryData::similar(sticker_unique_id, crate::inline::SimilarityAspect::Shape),
                    // )],
                    vec![InlineKeyboardButton::switch_inline_query_current_chat(
                        "🦄 Similar stickers (⚠️ ignores blacklist)",
                        InlineQueryData::similar(
                            sticker_id,
                            crate::inline::SimilarityAspect::Embedding,
                        ),
                    )],
                    vec![
                        InlineKeyboardButton::switch_inline_query_current_chat(
                            format!("🗂️ {set_count} {set_text} this sticker"),
                            InlineQueryData::sets(sticker_id.to_string()),
                        ),
                        InlineKeyboardButton::switch_inline_query_current_chat(
                            "🔧 Add to your set",
                            InlineQueryData::add_to_user_set(sticker_id.to_string()),
                        ),
                    ],
                ],
            ]
            .concat(),
        )
    }

    #[must_use]
    pub fn privacy(section: PrivacyPolicy) -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new(privacy_tabs(section))
    }

    pub fn continuous_tag_confirm(sticker_id: &str) -> InlineKeyboardMarkup {
        InlineKeyboardMarkup::new([[InlineKeyboardButton::callback(
            format!("Apply selected tags"),
            CallbackData::ApplyTags {
                sticker_id: sticker_id.to_string(),
            },
        )]])
    }
}

#[derive(PartialEq)]
enum StickerTab {
    Owner,
    Set,
    Sticker,
    Tags,
}

fn sticker_tabs(active: StickerTab, sticker_id: &str) -> Vec<InlineKeyboardButton> {
    let tabs = vec![
        (
            StickerTab::Tags,
            InlineKeyboardButton::callback(
                "🏷 Tags",
                CallbackData::Sticker {
                    sticker_id: sticker_id.to_string(),
                    operation: None,
                },
            ),
        ),
        (
            StickerTab::Sticker,
            InlineKeyboardButton::callback(
                "✨️ Sticker",
                CallbackData::StickerExplorePage {
                    sticker_id: sticker_id.to_string(),
                },
            ),
        ),
        (
            StickerTab::Set,
            InlineKeyboardButton::callback(
                "️🗂️ Set",
                CallbackData::StickerSetPage {
                    sticker_id: sticker_id.to_string(),
                },
            ),
        ),
        (
            StickerTab::Owner,
            InlineKeyboardButton::callback(
                "️👤 Owner",
                CallbackData::OwnerPage {
                    sticker_id: sticker_id.to_string(),
                },
            ),
        ),
    ];

    tabs.into_iter()
        .map(|(tab, button)| {
            let mut button = button;
            if tab == active {
                button.text = format!("▸ {} ◂", button.text);
            }
            button
        })
        .collect_vec()
}

#[derive(PartialEq)]
enum StatTab {
    General,
    Personal,
    Popular,
    LatestSets,
    LatestStickers,
    User,
}

fn stat_tabs(active: StatTab) -> Vec<InlineKeyboardButton> {
    let tabs = vec![
        (
            StatTab::General,
            InlineKeyboardButton::callback("🌐", CallbackData::GeneralStats),
        ),
        (
            StatTab::Personal,
            InlineKeyboardButton::callback("👤", CallbackData::PersonalStats),
        ),
        (
            StatTab::Popular,
            InlineKeyboardButton::callback("🏷", CallbackData::PopularTags),
        ),
        (
            StatTab::LatestSets,
            InlineKeyboardButton::callback("🗂️", CallbackData::LatestSets),
        ),
        (
            StatTab::LatestStickers,
            InlineKeyboardButton::callback("✨️", CallbackData::LatestStickers),
        ),
        (
            StatTab::User,
            InlineKeyboardButton::callback("👥", CallbackData::UserStats),
        ),
    ];

    tabs.into_iter()
        .map(|(tab, button)| {
            let mut button = button;
            if tab == active {
                button.text = format!("▸ {} ◂", button.text);
            }
            button
        })
        .collect_vec()
}

fn add_sticker_main_menu(
    sticker_id: &str,
    other_buttons: Vec<Vec<InlineKeyboardButton>>,
) -> Vec<Vec<InlineKeyboardButton>> {
    vec![sticker_tabs(StickerTab::Tags, sticker_id)]
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

async fn button_layout_to_keyboard_layout(
    button_layout: Vec<Vec<String>>,
    current_tags: &[String],
    sticker_unique_id: &str,
    tag_manager: TagManagerWorker,
) -> Result<Vec<Vec<InlineKeyboardButton>>, InternalError> {
    let keyboard = try_join_all(button_layout.iter().map(|row| async {
        Ok::<_, InternalError>(
            try_join_all(row.iter().map(|tag| async {
                Ok::<_, InternalError>(
                    tag_to_button(tag, current_tags, sticker_unique_id, tag_manager.clone())
                        .await?,
                )
            }))
            .await?,
        )
    }))
    .await?;
    Ok(keyboard)
}

async fn tag_to_button(
    tag: &str,
    current_tags: &[String],
    sticker_unique_id: &str,
    tag_manager: TagManagerWorker,
) -> Result<InlineKeyboardButton, InternalError> {
    let is_already_tagged = current_tags.contains(&tag.to_string());
    let callback_data = if is_already_tagged {
        CallbackData::untag_sticker(sticker_unique_id, tag.to_string())
    } else {
        CallbackData::tag_sticker(sticker_unique_id, tag.to_string())
    };
    let text = if is_already_tagged {
        // format!("✅ {}", tag.to_owned())
        let emoji = tag_manager
            .execute(GetCategory::new(tag.to_string()))
            .await?
            .map(|c| c.to_emoji())
            .unwrap_or("✅");
        format!("{} {}", emoji, tag.to_owned())
    } else {
        tag.to_owned()
    };
    Ok(InlineKeyboardButton::callback(
        text,
        callback_data.to_string(),
    ))
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

#[derive(PartialEq)]
enum HelpTab {
    Commands,
    Other,
}

fn help_tabs(active: HelpTab) -> Vec<Vec<InlineKeyboardButton>> {
    // vertical tabs
    let tabs = vec![
        (
            HelpTab::Commands,
            InlineKeyboardButton::callback("Commands", CallbackData::Help),
        ),
        (
            HelpTab::Other,
            InlineKeyboardButton::callback("Other Infos", CallbackData::Info),
        ),
    ];

    tabs.into_iter()
        .map(|(tab, button)| {
            let mut button = button;
            if tab == active {
                button.text = format!("▸ {} ◂", button.text);
            }
            vec![button]
        })
        .collect_vec()
}

fn privacy_tabs(active: PrivacyPolicy) -> Vec<Vec<InlineKeyboardButton>> {
    // vertical tabs
    let tabs = vec![
        PrivacyPolicy::Introduction,
        PrivacyPolicy::License,
        PrivacyPolicy::DataCollection,
        PrivacyPolicy::DataUsage,
    ]
    .into_iter()
    .map(|tab| {
        (
            tab,
            InlineKeyboardButton::callback(tab.title(), CallbackData::Privacy(Some(tab))),
        )
    });

    tabs.into_iter()
        .map(|(tab, button)| {
            let mut button = button;
            if tab == active {
                button.text = format!("▸ {} ◂", button.text);
            }
            vec![button]
        })
        .collect_vec()
}

fn can_insert_tag_in_column(current: &[String], tag: &str, icon_width: usize) -> bool {
    let final_button_count = current.len() + 1;
    let max_string_len = match final_button_count {
        0 | 1 => 9999,
        2 => 20,
        3 => 12,
        4 => 9,
        5 => 6,
        6 => 5,
        _ => 0,
    };

    current
        .iter()
        .all(|tag| tag.len() + icon_width <= max_string_len)
        && tag.len() + icon_width <= max_string_len
}
