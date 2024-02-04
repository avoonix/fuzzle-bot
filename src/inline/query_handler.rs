use crate::bot::Bot;
use crate::bot::{BotError, UserMeta};
use crate::database::Database;
use crate::inline::{InlineQueryData, InlineQueryDataMode, SetOperation};
use crate::message::StartParameter;
use crate::tags::TagManager;
use crate::worker::WorkerPool;
use itertools::Itertools;
use std::convert::TryFrom;
use std::sync::Arc;
use teloxide::{
    prelude::*,
    types::{
        InlineQueryResult, InlineQueryResultArticle, InlineQueryResultCachedSticker,
        InputMessageContent, InputMessageContentText,
    },
};
use url::Url;

use super::result_id::InlineQueryResultId;

const INLINE_QUERY_LIMIT: usize = 50;
const THUMBNAIL_SIZE: i32 = 200;

fn create_query_article(
    tag_manager: Arc<TagManager>,
    tag: &str,
    command_str: &str,
    description: &str,
) -> InlineQueryResult {
    let category = tag_manager.get_category(tag).unwrap_or_default();
    let color = category.to_color_name();
    let name = category.to_human_name();
    // TODO: do not rely on this service for images (base64 does not work)
    let thumbnail_url =
        format!("https://placehold.co/{THUMBNAIL_SIZE}/{color}/black.png?text={name}");
    let thumbnail_url = Url::parse(&thumbnail_url).unwrap();

    let content = InputMessageContent::Text(InputMessageContentText::new(command_str));

    InlineQueryResultArticle::new(
        InlineQueryResultId::Tag(tag.to_string()).to_string(),
        tag,
        content,
    )
    .thumb_url(thumbnail_url)
    .thumb_width(THUMBNAIL_SIZE)
    .thumb_height(THUMBNAIL_SIZE)
    .hide_url(true)
    .description(description)
    .into()
}

pub async fn inline_query_handler(
    bot: Bot,
    q: InlineQuery,
    tag_manager: Arc<TagManager>,
    worker: WorkerPool,
    database: Database,
    user: UserMeta,
) -> Result<(), BotError> {
    let query = match InlineQueryData::try_from(q.query) {
        Ok(query) => query,
        Err(err) => {
            dbg!(&err);
            bot.answer_inline_query(q.id, vec![])
                .switch_pm_text("Invalid query")
                .switch_pm_parameter(StartParameter::Help)
                // TODO: add start pm parameter "Help"
                .cache_time(600)
                .await?;
            return Ok(());
        }
    };

    let current_offset = q.offset.parse::<usize>().unwrap_or(0);
    match query.mode {
        InlineQueryDataMode::Set {
            set_name,
            operation,
        } => {
            // TODO: suggested tags for untag operation should only contain tags that the set contains
            let suggested_tags = tag_manager.find_tags(&query.tags);
            // TODO: if tags empty -> recommend tags from emojis/set name + fetch set
            let results = suggested_tags
                .into_iter()
                .skip(current_offset)
                .take(INLINE_QUERY_LIMIT) // TODO: proper pagination with 50 results
                .map(|tag| {
                    let (command, description) = match operation {
                        SetOperation::Tag => (
                            format!("/tagset {set_name} {tag}"),
                            "Tag ❗all stickers❗ in this set",
                        ),
                        SetOperation::Untag => (
                            format!("/untagset {set_name} {tag}"),
                            "Remove tag from ❗all stickers❗ in this set",
                        ),
                    };
                    create_query_article(tag_manager.clone(), &tag, &command, description)
                })
                .collect_vec();
            bot.answer_inline_query(q.id, results.clone())
                .next_offset(next_inline_query_offset(
                    results.len(),
                    current_offset,
                ))
                .cache_time(60) // in seconds // TODO: constant?
                .await?;
        }
        // suggest tags for a sticker
        InlineQueryDataMode::Sticker { unique_id } => {
            let tags = database.get_sticker_tags(unique_id.clone()).await?;
            // TODO: change query

            // TODO: if tags empty -> recommend tags

            // TODO: rename tag_manager
            let suggested_tags = tag_manager.find_tags(&query.tags);

            // TODO: do this in the database
            let suggested_tags = suggested_tags.into_iter().filter(|tag| !tags.contains(tag)); // do
                                                                                               // not
                                                                                               // recommend
                                                                                               // tags
                                                                                               // that
                                                                                               // are
                                                                                               // alraady
                                                                                               // added
                                                                                               // to
                                                                                               // the
                                                                                               // sticker
            let results = suggested_tags
                .into_iter()
                .skip(current_offset)
                .take(INLINE_QUERY_LIMIT) // TODO: proper pagination with 50 results
                .map(|tag| {
                    create_query_article(
                        tag_manager.clone(),
                        &tag,
                        &format!("/tagsticker {unique_id} {tag}"),
                        "Tag this sticker",
                    )
                })
                .collect_vec();
            bot.answer_inline_query(q.id, results.clone())
                .next_offset(next_inline_query_offset(
                    results.len(),
                    current_offset,
                ))
                .cache_time(0) // in seconds // TODO: constant?
                // .switch_pm_text("switch pm text") // TODO: use this?
                // .switch_pm_parameter("switchpmparameter")
                .await?;
        }
        // search for stickers
        InlineQueryDataMode::Search { emoji } => {
            // TODO: fall back to default blacklist if blacklist is not set
            let query_empty = query.tags.is_empty();

            // TODO: give warning: querying by emoji is not officially endorsed and very limited (no blacklist, only single emoji)

            let (stickers, empty_result_message) = if let Some(emoji) = emoji {
                (
                    database
                        .get_stickers_by_emoji(emoji, INLINE_QUERY_LIMIT, current_offset)
                        .await?,
                    "TODO".into(),
                )
            } else if query_empty {
                (
                    database
                        .get_recently_used_stickers(user.id().0, INLINE_QUERY_LIMIT, current_offset)
                        .await?,
                    "TODO".into(),
                )
            } else {
                // put everything from the query without - in the regular tag query, and everything with - in the blacklist
                let (tags, query_blacklist): (Vec<String>, Vec<String>) = query
                    .tags
                    .into_iter()
                    .partition(|tag| !tag.starts_with('-'));
                dbg!(&tags, &query_blacklist);
                let query_blacklist: Vec<String> = query_blacklist
                    .into_iter()
                    .map(|tag| tag.strip_prefix('-').unwrap_or(&tag).to_string())
                    .collect();
                let (tags, query_blacklist) = (
                    tag_manager.closest_matching_tags(&tags),
                    tag_manager.closest_matching_tags(&query_blacklist),
                );
                dbg!(&tags, &query_blacklist);

                let tags_empty = tags.is_empty();

                let blacklist = database
                    .get_blacklist(q.from.id.0)
                    .await?
                    .into_iter()
                    .chain(query_blacklist)
                    .collect_vec();

                let empty_result_message = if query_empty {
                    "Tip: try \"hug\"".to_string()
                } else if tags_empty {
                    "No matching tags found".to_string()
                } else {
                    format!("No results for \"{}\"", tags.join(" "))
                };

                // TODO: if tags are empty -> show the user's recently used or favorited (if implemented alread) stickers
                (
                    database
                        .get_stickers_for_tag_query(
                            tags.clone(),
                            blacklist,
                            INLINE_QUERY_LIMIT,
                            current_offset,
                        )
                        .await?,
                    empty_result_message,
                )
            };

            if !stickers.is_empty() {
                let random_sticker = rand::random::<usize>() % stickers.len();

                let random_sticker = stickers.get(random_sticker);
                if let Some(random_sticker) = random_sticker {
                    worker
                        .process_set_of_sticker(Some(user.id()), random_sticker.id.clone())
                        .await;
                }
            }

            let result_empty = stickers.is_empty();

            let sticker_result: Vec<InlineQueryResult> = stickers
                .into_iter()
                .map(|sticker| {
                    InlineQueryResultCachedSticker::new(
                        InlineQueryResultId::Sticker(sticker.id).to_string(),
                        sticker.file_id,
                    )
                    .into()
                })
                .collect();

            bot.answer_inline_query(q.id, sticker_result.clone())
                .next_offset(next_inline_query_offset(
                    sticker_result.len(),
                    current_offset,
                ))
                .cache_time(0)
                // this button could switch to a web app
                .switch_pm_text(if result_empty {
                    // TODO: message seems not to be the intended one sometimes
                    // TODO: no caching -> blacklist is personal for every user
                    empty_result_message
                    // else: open help
                    // else: searching: "<tags>" (with the actual resolved tags)
                } else {
                    "Edit Blacklist".to_string()
                }) // TODO: use this?
                .switch_pm_parameter(StartParameter::Blacklist.to_string())
                .await?;
        }
        InlineQueryDataMode::Blacklist => {
            let blacklist = database.get_blacklist(q.from.id.0).await?;
            let suggested_tags = tag_manager.find_tags(&query.tags);
            let suggested_tags = suggested_tags
                .into_iter()
                .filter(|tag| !blacklist.contains(tag));
            let results = suggested_tags
                .into_iter()
                .skip(current_offset)
                .take(INLINE_QUERY_LIMIT)
                .map(|tag| {
                    create_query_article(
                        tag_manager.clone(),
                        &tag,
                        &format!("/blacklisttag {tag}"),
                        "Blacklist this tag",
                    )
                })
                .collect_vec();

            bot.answer_inline_query(q.id, results.clone())
                .next_offset(next_inline_query_offset(
                    results.len(),
                    current_offset,
                ))
                .cache_time(0) // in seconds // TODO: constant?
                .await?;
        }
        InlineQueryDataMode::ContinuousTagMode { operation } => {
            // TODO: add undo button after every tagging
            let suggested_tags = tag_manager.find_tags(&query.tags);
            let results = suggested_tags
                .into_iter()
                .skip(current_offset)
                .take(INLINE_QUERY_LIMIT)
                .map(|tag| {
                    let (command, description) = match operation {
                        SetOperation::Tag => (
                            format!("/tagcontinuous {tag}"),
                            "Tag multiple stickers in a row",
                        ),
                        SetOperation::Untag => (
                            format!("/untagcontinuous {tag}"),
                            "Remove tag from multiple stickers in a row",
                        ),
                    };
                    create_query_article(tag_manager.clone(), &tag, &command, description)
                })
                .collect_vec();

            bot.answer_inline_query(q.id, results.clone())
                .next_offset(next_inline_query_offset(
                    results.len(),
                    current_offset,
                ))
                .cache_time(300) // in seconds // TODO: constant?
                .await?;
        }
    }

    Ok(())
}

fn next_inline_query_offset(current_result_len: usize, current_offset: usize) -> String {
    if current_result_len >= INLINE_QUERY_LIMIT {
        (current_offset + INLINE_QUERY_LIMIT).to_string()
    } else {
        String::new() // empty string means no more results
    }
}
