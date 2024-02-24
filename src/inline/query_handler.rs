use crate::background_tasks::BackgroundTaskExt;
use crate::bot::{BotExt, RequestContext};
use crate::bot::{BotError, UserMeta};
use crate::database::{Database, SavedSticker};
use crate::inline::{InlineQueryData, InlineQueryDataMode, SetOperation};
use crate::message::StartParameter;
use crate::sticker::{compute_similar, find_with_text_embedding};
use crate::tags::TagManager;
use crate::text::{Markdown, Text};
use crate::util::Emoji;
use itertools::Itertools;
use log::warn;
use std::convert::TryFrom;
use std::sync::Arc;
use teloxide::types::Recipient;
use teloxide::{
    prelude::*,
    types::{
        InlineQueryResult, InlineQueryResultArticle, InlineQueryResultCachedSticker,
        InputMessageContent, InputMessageContentText,
    },
};
use url::Url;

use super::pagination::QueryPage;
use super::result_id::InlineQueryResultId;
use super::SimilarityAspect;

// TODO: seems like switch_pm_text can not be updated dynamically (eg to abuse it and show the number of results, resolved tags, etc) -> find other way to show that info

const INLINE_QUERY_LIMIT: usize = 50;
const THUMBNAIL_SIZE: i32 = 200;

fn create_query_article(
    tag_manager: Arc<TagManager>,
    tag: &str,
    command_str: &str,
    description: &str,
) -> Result<InlineQueryResult, BotError> {
    let category = tag_manager.get_category(tag).unwrap_or_default();
    let color = category.to_color_name();
    let name = category.to_human_name();
    // TODO: do not rely on this service for images (base64 does not work)
    let thumbnail_url =
        format!("https://placehold.co/{THUMBNAIL_SIZE}/{color}/black.png?text={name}");
    let thumbnail_url = Url::parse(&thumbnail_url)?;

    let content = InputMessageContent::Text(InputMessageContentText::new(command_str));

    Ok(InlineQueryResultArticle::new(
        InlineQueryResultId::Tag(tag.to_string()).to_string(),
        tag,
        content,
    )
    .thumb_url(thumbnail_url)
    .thumb_width(THUMBNAIL_SIZE)
    .thumb_height(THUMBNAIL_SIZE)
    .hide_url(true)
    .description(description)
    .into())
}

async fn handle_set_query(
    current_offset: QueryPage,
    query: InlineQueryData,
    operation: SetOperation,
    set_name: String,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    // TODO: suggested tags for untag operation should only contain tags that the set contains
    let suggested_tags = request_context.tag_manager.find_tags(&query.tags);
    // TODO: if tags empty -> recommend tags from emojis/set name + fetch set
    let results = suggested_tags
        .into_iter()
        .skip(current_offset.skip())
        .take(current_offset.page_size())
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
            create_query_article(
                request_context.tag_manager.clone(),
                &tag,
                &command,
                description,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    request_context
        .bot
        .answer_inline_query(q.id, results.clone())
        .next_offset(current_offset.next_query_offset(results.len()))
        .cache_time(60)
        .await?;
    Ok(())
}

async fn handle_sticker_query(
    current_offset: QueryPage,
    query: InlineQueryData,
    unique_id: String,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let tags = request_context
        .database
        .get_sticker_tags(unique_id.clone())
        .await?;
    // TODO: change query

    // TODO: if tags empty -> recommend tags

    let suggested_tags = request_context.tag_manager.find_tags(&query.tags);

    let suggested_tags = suggested_tags.into_iter().filter(|tag| !tags.contains(tag));
    let results = suggested_tags
        .into_iter()
        .skip(current_offset.skip())
        .take(current_offset.page_size())
        .map(|tag| {
            create_query_article(
                request_context.tag_manager.clone(),
                &tag,
                &format!("/tagsticker {unique_id} {tag}"),
                "Tag this sticker",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    request_context
        .bot
        .answer_inline_query(q.id, results.clone())
        .next_offset(current_offset.next_query_offset(results.len()))
        .cache_time(0)
        .await?;
    Ok(())
}

pub async fn query_stickers(
    query: InlineQueryData,
    database: Database,
    emoji: Option<Emoji>,
    user: Arc<UserMeta>,
    tag_manager: Arc<TagManager>,
    limit: usize,
    offset: usize,
    seed: i32,
) -> Result<Vec<SavedSticker>, BotError> {
    // TODO: fall back to default blacklist if blacklist is not set
    let query_empty = query.tags.is_empty();

    // TODO: give warning: querying by emoji is very limited (no blacklist, only single emoji)

    let order = user.user.settings.order();
    let order = match order {
        crate::database::StickerOrder::LatestFirst => crate::database::Order::LatestFirst,
        crate::database::StickerOrder::Random => crate::database::Order::Random { seed },
    };

    let stickers = if let Some(emoji) = emoji {
        database.get_stickers_by_emoji(emoji, limit, offset).await?
    } else if query_empty {
        let stickers = database
            .get_recently_used_stickers(user.id().0, limit, offset)
            .await?;
        if stickers.is_empty() {
            database
                .get_stickers_for_tag_query(vec![], user.user.blacklist.clone(), limit, offset, order)
                .await?
        } else {
            stickers
        }
    } else {
        let (tags, query_blacklist): (Vec<String>, Vec<String>) = query
            .tags
            .into_iter()
            .partition(|tag| !tag.starts_with('-'));
        let query_blacklist: Vec<String> = query_blacklist
            .into_iter()
            .map(|tag| tag.strip_prefix('-').unwrap_or(&tag).to_string())
            .collect(); // TODO: this should probably be done during parsing
        let (tags, query_blacklist) = (
            tag_manager.closest_matching_tags(&tags),
            tag_manager.closest_matching_tags(&query_blacklist),
        );

        let tags_empty = tags.is_empty();

        let blacklist = user
            .user
            .blacklist
            .iter()
            .cloned()
            .chain(query_blacklist)
            .collect_vec();

        // let empty_result_message = if query_empty {
        //     "Tip: try \"hug\"".to_string()
        // } else if tags_empty {
        //     "No matching tags found".to_string()
        // } else {
        //     format!("No results for \"{}\"", tags.join(" "))
        // };

        // TODO: if tags are empty -> show the user's recently used or favorited (if implemented alread) stickers
        database
            .get_stickers_for_tag_query(tags.clone(), blacklist, limit, offset, order)
            .await?
    };

    Ok(stickers)
}

async fn handle_similar_sticker_query(
    current_offset: QueryPage,
    query: InlineQueryData,
    sticker_unique_id: String,
    aspect: SimilarityAspect,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    // TODO: cache?
    // TODO: blacklist?
    let result = compute_similar(
        request_context.database.clone(),
        sticker_unique_id,
        aspect,
        request_context.analysis_worker,
        current_offset.skip() + current_offset.page_size()
    )
    .await?
    .items();
    let sticker_ids = result
        .into_iter()
        .map(|m| m.sticker_id)
        .skip(current_offset.skip())
        .take(current_offset.page_size())
        .collect_vec();
    let mut stickers = Vec::new();
    for id in sticker_ids {
        stickers.push(request_context.database.get_sticker(id).await?); // TODO: single query?
    }

    let sticker_result = stickers
        .into_iter()
        .flatten()
        .map(|sticker| {
            InlineQueryResultCachedSticker::new(
                InlineQueryResultId::Sticker(sticker.id).to_string(),
                sticker.file_id,
            )
            .into()
        })
        .collect_vec();

    request_context
        .bot
        .answer_inline_query(q.id, sticker_result.clone())
        .next_offset(current_offset.next_query_offset(sticker_result.len()))
        .cache_time(0)
        .switch_pm_text(Text::switch_pm_text())
        .switch_pm_parameter(StartParameter::Greeting.to_string())
        .is_personal(true)
        .await?;
    Ok(())
}

async fn handle_sticker_search(
    current_offset: QueryPage,
    query: InlineQueryData,
    emoji: Option<Emoji>,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let stickers = query_stickers(
        query,
        request_context.database.clone(),
        emoji,
        request_context.user.clone(),
        request_context.tag_manager.clone(),
        current_offset.page_size(),
        current_offset.skip(),
        current_offset.seed(),
    )
    .await?;

    if !stickers.is_empty() {
        let random_sticker = rand::random::<usize>() % stickers.len();

        let random_sticker = stickers.get(random_sticker);
        if let Some(random_sticker) = random_sticker {
            request_context
                .process_set_of_sticker(random_sticker.id.clone())
                .await;
        }
    }

    let result_empty = stickers.is_empty();

    let sticker_result = stickers
        .into_iter()
        .map(|sticker| {
            InlineQueryResultCachedSticker::new(
                InlineQueryResultId::Sticker(sticker.id).to_string(),
                sticker.file_id,
            )
            .into()
        })
        .collect_vec();

    request_context
        .bot
        .answer_inline_query(q.id, sticker_result.clone())
        .next_offset(current_offset.next_query_offset(sticker_result.len()))
        .cache_time(0)
        // TODO: this button could switch to a web app; or the blacklist start parameter
        // also it should be possible to display the actually resolved tags (maybe when teloxide implements the new bot api)
        .switch_pm_text(Text::switch_pm_text())
        .switch_pm_parameter(StartParameter::Greeting.to_string())
        .is_personal(true)
        .await?;
    Ok(())
}

async fn handle_blacklist_query(
    current_offset: QueryPage,
    query: InlineQueryData,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let blacklist = &request_context.user.user.blacklist;
    let suggested_tags = request_context.tag_manager.find_tags(&query.tags);
    let suggested_tags = suggested_tags
        .into_iter()
        .filter(|tag| !blacklist.contains(tag));
    let results = suggested_tags
        .into_iter()
        .skip(current_offset.skip())
        .take(current_offset.page_size())
        .map(|tag| {
            create_query_article(
                request_context.tag_manager.clone(),
                &tag,
                &format!("/blacklisttag {tag}"),
                "Blacklist this tag",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    request_context
        .bot
        .answer_inline_query(q.id, results.clone())
        .next_offset(current_offset.next_query_offset(results.len()))
        .cache_time(0) // in seconds // TODO: constant?
        .await?;

    Ok(())
}

async fn handle_continuous_tag_query(
    current_offset: QueryPage,
    query: InlineQueryData,
    operation: SetOperation,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    // TODO: add undo button after every tagging
    let suggested_tags = request_context.tag_manager.find_tags(&query.tags);
    let results = suggested_tags
        .into_iter()
        .skip(current_offset.skip())
        .take(current_offset.page_size())
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
            create_query_article(
                request_context.tag_manager.clone(),
                &tag,
                &command,
                description,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    request_context
        .bot
        .answer_inline_query(q.id, results.clone())
        .next_offset(current_offset.next_query_offset(results.len()))
        .cache_time(300) // in seconds // TODO: constant?
        .await?;

    Ok(())
}

pub async fn inline_query_handler(
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let in_bot_chat = q.chat_type.unwrap_or(teloxide::types::ChatType::Sender)
        == teloxide::types::ChatType::Sender;

    let query = match InlineQueryData::try_from(q.query.clone()) {
        Ok(query) => query,
        Err(err) => {
            warn!("{err}");
            InlineQueryData::search(vec![])
            // TODO: somehow notify user of error (seems impossible with this version of the bot api, unless I maybe use articles?)
        }
    };

    let current_offset = QueryPage::from_query_offset(&q.offset, INLINE_QUERY_LIMIT)?;
    match query.mode.clone() {
        InlineQueryDataMode::Set {
            set_name,
            operation,
        } => {
            handle_set_query(
                current_offset,
                query,
                operation,
                set_name,
                q,
                request_context,
            )
            .await
        }
        InlineQueryDataMode::Sticker { unique_id } => {
            handle_sticker_query(current_offset, query, unique_id, q, request_context).await
        }
        InlineQueryDataMode::StickerSearch { emoji } => {
            handle_sticker_search(current_offset, query, emoji, q, request_context).await
        }
        InlineQueryDataMode::Blacklist => {
            handle_blacklist_query(current_offset, query, q, request_context).await
        }
        InlineQueryDataMode::ContinuousTagMode { operation } => {
            handle_continuous_tag_query(current_offset, query, operation, q, request_context).await
        }
        InlineQueryDataMode::Similar { unique_id, aspect } => {
            handle_similar_sticker_query(
                current_offset,
                query,
                unique_id,
                aspect,
                q,
                request_context,
            )
            .await
        }
        InlineQueryDataMode::EmbeddingSearch => {
            handle_embedding_query(current_offset, query.tags.join(" "), q, request_context).await
        }
    }
}

async fn handle_embedding_query(
    current_offset: QueryPage,
    query: String,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    // TODO: cache?
    // TODO: blacklist?

    // dbg!(result);

    let result = find_with_text_embedding(
        request_context.database.clone(),
        query,
        request_context.analysis_worker,
        current_offset.skip() + current_offset.page_size(),
    )
    .await;
    let result = match result {
        Ok(result) => result,
        Err(BotError::Embedding(embedding_error)) => {
            warn!("{embedding_error:?}");
            // TODO: use central error handler?
            request_context
                .bot
                .answer_inline_query(q.id, vec![])
                .await?;
            request_context
                .bot
                .send_markdown(
                    Recipient::Id(q.from.id.into()),
                    Markdown::escaped(match embedding_error {
                        crate::sticker::EmbeddingError::UnknownToken(token) => {
                            format!("unknown token {token}")
                        }
                        _ => "an error occured".to_string(),
                    }),
                )
                .await?;
            return Ok(());
        }
        Err(err) => return Err(err),
    };
    let sticker_ids = result
        .items()
        .into_iter()
        .map(|m| m.sticker_id)
        .skip(current_offset.skip())
        .take(current_offset.page_size())
        .collect_vec();
    let mut stickers = Vec::new();
    for id in sticker_ids {
        stickers.push(request_context.database.get_sticker(id).await?); // TODO: single query?
    }

    let sticker_result = stickers
        .into_iter()
        .flatten()
        .map(|sticker| {
            InlineQueryResultCachedSticker::new(
                InlineQueryResultId::Sticker(sticker.id).to_string(),
                sticker.file_id,
            )
            .into()
        })
        .collect_vec();

    request_context
        .bot
        .answer_inline_query(q.id, sticker_result.clone())
        .next_offset(current_offset.next_query_offset(sticker_result.len()))
        .cache_time(0)
        .switch_pm_text(Text::switch_pm_text())
        .switch_pm_parameter(StartParameter::Greeting.to_string())
        .is_personal(true)
        .await?;
    Ok(())
}
