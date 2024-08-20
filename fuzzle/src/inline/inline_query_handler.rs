use crate::background_tasks::BackgroundTaskExt;
use crate::bot::{
    report_bot_error, report_internal_error, report_internal_error_result, BotError, InternalError,
    UserError,
};
use crate::bot::{BotExt, RequestContext};
use crate::database::{self, min_max, DialogState, User};
use crate::database::{Database, Sticker, StickerSet};
use crate::inline::{InlineQueryData, SetOperation};
use crate::message::{Keyboard, StartParameter};
use crate::sticker::{compute_similar, find_with_text_embedding, with_sticker_id, Match};
use crate::tags::TagManager;
use crate::text::{Markdown, Text};
use crate::util::{create_sticker_set_id, create_tag_id, format_relative_time, Emoji, Required};
use itertools::Itertools;
use num_traits::ToPrimitive;
use std::convert::TryFrom;
use std::future::IntoFuture;
use std::sync::Arc;
use teloxide::types::Recipient;
use teloxide::utils::markdown::escape;
use teloxide::{
    prelude::*,
    types::{
        InlineQueryResult, InlineQueryResultArticle, InlineQueryResultCachedSticker,
        InputMessageContent, InputMessageContentText,
    },
};
use tracing::{warn, Instrument};
use url::Url;

use super::pagination::QueryPage;
use super::result_id::InlineQueryResultId;
use super::{SimilarityAspect, TagKind};

// TODO: seems like switch_pm_text can not be updated dynamically (eg to abuse it and show the number of results, resolved tags, etc) -> find other way to show that info

const INLINE_QUERY_LIMIT: usize = 50;
const THUMBNAIL_SIZE: u32 = 200;

#[tracing::instrument(skip(set))]
fn create_query_set(
    set: &database::StickerSet,
    info: Option<String>,
    thumb_url: String,
) -> Result<InlineQueryResult, BotError> {
    // // TODO: do not rely on this service for images (base64 does not work)
    let set_title = set.title.clone().unwrap_or(set.id.clone());

    let content = InputMessageContent::Text(InputMessageContentText::new(
        Text::get_set_article_link(&set.id, &set_title),
    ));

    let mut article = InlineQueryResultArticle::new(
        InlineQueryResultId::Set(set.id.clone()).to_string(),
        set_title,
        content,
    )
    .description(info.map_or(set.id.clone(), |info| format!("{} • {}", set.id, info)));
    // let thumbnail_url =
    // format!("https://placehold.co/{THUMBNAIL_SIZE}/007f0e/black.png?text={thumb}");
    let thumbnail_url = Url::parse(&thumb_url)?;
    article = article
        .thumb_url(thumbnail_url)
        .thumb_width(THUMBNAIL_SIZE)
        .thumb_height(THUMBNAIL_SIZE)
        .hide_url(true);

    Ok(article.into())
}

#[tracing::instrument(skip(tag_manager))]
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

    let content = InputMessageContent::Text(InputMessageContentText::new(escape(command_str)));

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

fn treat_missing_tags_as_errors(
    closest_tags: Vec<(String, Option<String>)>,
) -> Result<Vec<String>, UserError> {
    let missing = closest_tags
        .iter()
        .filter_map(|(input, output)| {
            if output.is_none() {
                Some(input.clone())
            } else {
                None
            }
        })
        .collect_vec();
    if missing.is_empty() {
        Ok(closest_tags
            .into_iter()
            .filter_map(|(_, output)| output)
            .collect_vec())
    } else {
        Err(UserError::TagsNotFound(missing))
    }
}

fn get_last_input_match_list_and_other_input_closest_matches(
    tags: Vec<Vec<String>>,
    tag_manager: Arc<TagManager>,
) -> Result<(Vec<String>, Vec<String>), UserError> {
    if tags.is_empty() {
        return Ok((vec![], tag_manager.find_tags(&[])));
    }
    let last_input = tags.last().cloned().unwrap_or_default();
    let other_len = tags.len() - 1;
    let other_input = tags
        .into_iter()
        .take(other_len)
        .map(|parts| parts.join("_"))
        .collect_vec();
    let other_input =
        treat_missing_tags_as_errors(tag_manager.closest_matching_tags(&other_input))?;

    let suggested_tags = tag_manager.find_tags(&last_input);

    Ok((other_input, suggested_tags))
}

#[tracing::instrument(skip(q, request_context))]
async fn search_tags_for_sticker_set(
    current_offset: QueryPage,
    tags: Vec<Vec<String>>,
    operation: SetOperation,
    set_name: String,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    // TODO: suggested tags for untag operation should only contain tags that the set contains
    let (other_tags, suggested_tags) = get_last_input_match_list_and_other_input_closest_matches(
        tags,
        request_context.tag_manager.clone(),
    )?;
    // TODO: if tags empty -> recommend tags from emojis/set name + fetch set
    let results = suggested_tags
        .into_iter()
        .skip(current_offset.skip())
        .take(current_offset.page_size())
        .map(|tag| {
            let all_tags_list = other_tags.iter().chain(std::iter::once(&tag)).join(",");
            let (command, description) = match operation {
                SetOperation::Tag => (
                    format!("/tagset {set_name} {all_tags_list}"),
                    format!("❗all stickers in set❗ add {all_tags_list}"),
                ),
                SetOperation::Untag => (
                    format!("/untagset {set_name} {all_tags_list}"),
                    format!("❗all stickers in set❗ remove {all_tags_list}"),
                ),
            };
            create_query_article(
                request_context.tag_manager.clone(),
                &tag,
                &command,
                &description,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    require_some_results("tags", current_offset, results.len())?;
    request_context
        .bot
        .answer_inline_query(q.id, results.clone())
        .next_offset(current_offset.next_query_offset(results.len()))
        .cache_time(60)
        .await?;
    Ok(())
}

fn require_some_results(
    name: &str,
    offset: QueryPage,
    current_result_count: usize,
) -> Result<(), UserError> {
    if offset.is_first_page() && current_result_count == 0 {
        Err(UserError::ListHasZeroResults(name.to_string()))
    } else {
        Ok(())
    }
}

#[tracing::instrument(skip(q, request_context))]
async fn search_tags_for_sticker(
    current_offset: QueryPage,
    tags: Vec<Vec<String>>,
    unique_id: String,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let sticker_tags = request_context
        .database
        .get_sticker_tags(&unique_id)
        .await?;
    // TODO: change query

    // TODO: if tags empty -> recommend tags

    let (other_tags, suggested_tags) = get_last_input_match_list_and_other_input_closest_matches(
        tags,
        request_context.tag_manager.clone(),
    )?;

    let suggested_tags = suggested_tags
        .into_iter()
        .filter(|tag| !sticker_tags.contains(tag));
    let results = suggested_tags
        .into_iter()
        .skip(current_offset.skip())
        .take(current_offset.page_size())
        .map(|tag| {
            let all_tags_list = other_tags.iter().chain(std::iter::once(&tag)).join(",");
            create_query_article(
                request_context.tag_manager.clone(),
                &tag,
                &format!("/tagsticker {unique_id} {all_tags_list}"),
                &format!("Tag this sticker: {all_tags_list}"),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    require_some_results("tags", current_offset, results.len())?;
    request_context
        .bot
        .answer_inline_query(q.id, results.clone())
        .next_offset(current_offset.next_query_offset(results.len()))
        .cache_time(0)
        .await?;
    Ok(())
}

#[tracing::instrument(skip(database, user, tag_manager))]
pub async fn query_stickers(
    tags: Vec<String>,
    database: Database,
    emoji: Vec<Emoji>,
    user: Arc<User>,
    tag_manager: Arc<TagManager>,
    limit: usize,
    offset: usize,
    seed: i32,
) -> Result<Vec<Sticker>, BotError> {
    // TODO: fall back to default blacklist if blacklist is not set
    let query_empty = tags.is_empty() && emoji.is_empty();

    // TODO: give warning: querying by emoji is very limited (no blacklist, only single emoji)

    let order = user.settings.clone().unwrap_or_default().order();
    let order = match order {
        crate::database::StickerOrder::LatestFirst => crate::database::Order::LatestFirst,
        crate::database::StickerOrder::Random => crate::database::Order::Random { seed },
    };

    let emoji = emoji
        .into_iter()
        .map(|emoji| emoji.to_string_without_variant())
        .collect_vec();

    let stickers = if query_empty {
        let stickers = database
            .get_recently_used_stickers(user.id, limit as i64, offset as i64)
            .await?;
        if stickers.is_empty() {
            database
                .get_stickers_for_tag_query(
                    vec![],
                    user.blacklist.clone().into_inner(),
                    vec![],
                    limit as i64,
                    offset as i64,
                    order,
                )
                .await?
        } else {
            stickers
        }
    } else {
        let (tags, query_blacklist): (Vec<String>, Vec<String>) =
            tags.into_iter().partition(|tag| !tag.starts_with('-'));
        if query_blacklist.is_empty() && tags.is_empty() && emoji.len() == 1 {
            // TODO: warn the user that this is not blacklisted
            return Ok(database
                .get_stickers_by_emoji(&emoji[0].to_string(), limit as i64, offset as i64)
                .await?);
        }
        let query_blacklist: Vec<String> = query_blacklist
            .into_iter()
            .map(|tag| tag.strip_prefix('-').unwrap_or(&tag).to_string())
            .collect(); // TODO: this should probably be done during parsing
        let (tags, query_blacklist) = (
            treat_missing_tags_as_errors(tag_manager.closest_matching_tags(&tags))?,
            treat_missing_tags_as_errors(tag_manager.closest_matching_tags(&query_blacklist))?,
        );

        let tags_empty = tags.is_empty();

        let blacklist = user
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
            .get_stickers_for_tag_query(
                tags.clone(),
                blacklist,
                emoji,
                limit as i64,
                offset as i64,
                order,
            )
            .await?
    };

    Ok(stickers)
}

#[tracing::instrument(skip(query, q, request_context))]
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
    let (result, original_len) = compute_similar(
        request_context.clone(),
        sticker_unique_id,
        aspect,
        current_offset.page_size() as u64,
        current_offset.skip() as u64,
    )
    .await?;
    let sticker_ids = result.into_iter().map(|m| m.sticker_id).collect_vec();
    let mut stickers = Vec::new();
    for id in sticker_ids {
        stickers.push(request_context.database.get_sticker_by_id(&id).await?); // TODO: single query?
    }

    let sticker_result = stickers
        .into_iter()
        .flatten()
        .map(|sticker| {
            InlineQueryResultCachedSticker::new(
                InlineQueryResultId::Sticker(sticker.id).to_string(),
                sticker.telegram_file_identifier,
            )
            .into()
        })
        .collect_vec();

    require_some_results("stickers", current_offset, original_len)?;
    request_context
        .bot
        .answer_inline_query(q.id, sticker_result.clone())
        .next_offset(current_offset.next_query_offset(original_len))
        .cache_time(0)
        .switch_pm_text(Text::switch_pm_text())
        .switch_pm_parameter(StartParameter::Greeting.to_string())
        .is_personal(true)
        .await?;
    Ok(())
}

#[tracing::instrument(skip(request_context, q))]
async fn search_stickers(
    current_offset: QueryPage,
    tags: Vec<String>,
    emoji: Vec<Emoji>,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let stickers = query_stickers(
        tags,
        request_context.database.clone(),
        emoji,
        request_context.user.clone(),
        request_context.tag_manager.clone(),
        current_offset.page_size(),
        current_offset.skip(),
        current_offset.seed(),
    )
    .await?;

    let result_empty = stickers.is_empty();

    let sticker_result = stickers
        .into_iter()
        .map(|sticker| {
            InlineQueryResultCachedSticker::new(
                InlineQueryResultId::Sticker(sticker.id).to_string(),
                sticker.telegram_file_identifier,
            )
            .into()
        })
        .collect_vec();

    require_some_results("stickers", current_offset, sticker_result.len())?;
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
        .into_future()
        .instrument(tracing::info_span!("telegram_bot_answer_inline_query"))
        .await?;
    Ok(())
}

#[tracing::instrument(skip(q, request_context))]
async fn search_tags_for_blacklist(
    current_offset: QueryPage,
    tags: Vec<String>,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let blacklist = &request_context.user.blacklist;
    let suggested_tags = request_context.tag_manager.find_tags(&tags);
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

    require_some_results("tags", current_offset, results.len())?;
    request_context
        .bot
        .answer_inline_query(q.id, results.clone())
        .next_offset(current_offset.next_query_offset(results.len()))
        .cache_time(0) // in seconds // TODO: constant?
        .await?;

    Ok(())
}

#[tracing::instrument(skip(q, request_context))]
async fn handle_continuous_tag_query(
    current_offset: QueryPage,
    tags: Vec<Vec<String>>,
    operation: SetOperation,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let (other_tags, suggested_tags) = get_last_input_match_list_and_other_input_closest_matches(
        tags,
        request_context.tag_manager.clone(),
    )?;
    let results = suggested_tags
        .into_iter()
        .skip(current_offset.skip())
        .take(current_offset.page_size())
        .map(|tag| {
            let all_tags_list = other_tags.iter().chain(std::iter::once(&tag)).join(",");
            let (command, description) = match operation {
                SetOperation::Tag => (
                    format!("/tagcontinuous {all_tags_list}"),
                    format!("Add tags: {all_tags_list}"),
                ),
                SetOperation::Untag => (
                    format!("/untagcontinuous {all_tags_list}"),
                    format!("Remove tags: {all_tags_list}"),
                ),
            };
            create_query_article(
                request_context.tag_manager.clone(),
                &tag,
                &command,
                &description,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    require_some_results("tags", current_offset, results.len())?;
    request_context
        .bot
        .answer_inline_query(q.id, results.clone())
        .next_offset(current_offset.next_query_offset(results.len()))
        .cache_time(300) // in seconds // TODO: constant?
        .await?;

    Ok(())
}

#[tracing::instrument(skip(request_context))]
pub async fn inline_query_handler_wrapper(
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), ()> {
    match inline_query_handler(q.clone(), request_context.clone()).await {
        Ok(_) => {}
        Err(error) => {
            report_bot_error(&error);
            report_internal_error_result(show_error(q, request_context, error).await);
        }
    }
    Ok(())
}

#[tracing::instrument(skip(request_context, q), err(Debug))]
pub async fn show_error(
    q: InlineQuery,
    request_context: RequestContext,
    error: BotError,
) -> Result<(), BotError> {
    let error = error.end_user_error();
    let thumbnail_url = match error.1 {
        crate::bot::UserErrorSeverity::Error => Url::parse(&format!(
            "https://fuzzle-bot.avoonix.com/assets/fuzzle_error.png"
        ))?,
        crate::bot::UserErrorSeverity::Info => Url::parse(&format!(
            "https://fuzzle-bot.avoonix.com/assets/fuzzle_info.png"
        ))?,
    };

    let content =
        InputMessageContent::Text(InputMessageContentText::new(Markdown::escaped(&error.0)));

    let error_message = InlineQueryResultArticle::new(
        InlineQueryResultId::Other("error".to_string()).to_string(),
        &error.0,
        content,
    )
    .thumb_url(thumbnail_url)
    .thumb_width(THUMBNAIL_SIZE)
    .thumb_height(THUMBNAIL_SIZE)
    .hide_url(true)
    .into();

    request_context
        .bot
        .answer_inline_query(q.id, vec![error_message])
        .next_offset("")
        .cache_time(0)
        .switch_pm_text(Text::switch_pm_text())
        .switch_pm_parameter(StartParameter::Greeting.to_string())
        .is_personal(true)
        .await?;
    Ok(())
}

#[tracing::instrument(skip(request_context, q), err(Debug))]
pub async fn inline_query_handler(
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let in_bot_chat = q.chat_type.unwrap_or(teloxide::types::ChatType::Sender)
        == teloxide::types::ChatType::Sender;

    let query = InlineQueryData::try_from(q.query.clone())?;

    let current_offset = QueryPage::from_query_offset(&q.offset, INLINE_QUERY_LIMIT)?;
    match query.clone() {
        InlineQueryData::SearchTagsForStickerSet {
            set_name,
            operation,
            tags,
        } => {
            search_tags_for_sticker_set(
                current_offset,
                tags,
                operation,
                set_name,
                q,
                request_context,
            )
            .await
        }
        InlineQueryData::SearchTagsForSticker { unique_id, tags } => {
            search_tags_for_sticker(current_offset, tags, unique_id, q, request_context).await
        }
        InlineQueryData::SearchStickers { emoji, tags } => {
            search_stickers(current_offset, tags, emoji, q, request_context).await
        }
        InlineQueryData::SearchTagsForBlacklist { tags } => {
            search_tags_for_blacklist(current_offset, tags, q, request_context).await
        }
        InlineQueryData::SearchTagsForContinuousTagMode { operation, tags } => {
            handle_continuous_tag_query(current_offset, tags, operation, q, request_context).await
        }
        InlineQueryData::TagCreatorTagId { tag_id, kind } => {
            handle_tag_creator(q, request_context, tag_id, kind).await
        }
        InlineQueryData::ListSimilarStickers { unique_id, aspect } => {
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
        InlineQueryData::SearchByEmbedding { query } => {
            handle_embedding_query(current_offset, query, q, request_context).await
        }
        InlineQueryData::ListMostDuplicatedStickers => {
            handle_most_duplicated_stickers(current_offset, q, request_context).await
        }
        InlineQueryData::ListMostUsedEmojis => {
            handle_most_used_emojis(current_offset, q, request_context).await
        }
        InlineQueryData::ListRecommendationModeRecommendations => {
            get_recommended_stickers_in_recommender_mode(current_offset, q, request_context).await
        }
        InlineQueryData::ListAllSetsThatContainSticker { sticker_id } => {
            handle_sticker_contained_query(current_offset, sticker_id, q, request_context).await
        }
        InlineQueryData::ListOverlappingSets { sticker_id } => {
            handle_overlapping_sets(current_offset, sticker_id, q, request_context).await
        }
        InlineQueryData::SetsByUserId { user_id } => {
            get_all_sticker_sets_owned_by_a_user(current_offset, user_id, q, request_context).await
        }
        InlineQueryData::ListSetStickersByDate { sticker_id } => {
            handle_stickers_by_date(current_offset, sticker_id, q, request_context).await
        }
        InlineQueryData::ListAllTagsFromSet { sticker_id } => {
            handle_all_set_tags(current_offset, sticker_id, q, request_context).await
        }
        InlineQueryData::AddToUserSet {
            sticker_id,
            set_title,
        } => handle_user_sets(current_offset, sticker_id, set_title, q, request_context).await,
    }
}

#[tracing::instrument(skip(q, request_context))]
async fn handle_sticker_contained_query(
    current_offset: QueryPage,
    sticker_id: String,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let file = request_context
        .database
        .get_sticker_file_by_sticker_id(&sticker_id)
        .await?
        .required()?;
    let sets = request_context
        .database
        .get_sets_containing_file(&file.id)
        .await?;

    let results = sets
        .into_iter()
        .skip(current_offset.skip())
        .take(current_offset.page_size())
        .map(|set| {
            create_query_set(
                &set,
                None,
                format!(
                    "https://fuzzle-bot.avoonix.com/thumbnails/sticker-set/{}/image.png",
                    &set.id
                ),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    require_some_results("sets", current_offset, results.len())?;
    request_context
        .bot
        .answer_inline_query(q.id, results.clone())
        .next_offset(current_offset.next_query_offset(results.len()))
        .cache_time(0) // in seconds // TODO: constant?
        .await?;

    Ok(())
}

#[tracing::instrument(skip(q, request_context))]
async fn handle_all_set_tags(
    current_offset: QueryPage,
    sticker_id: String,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let set = request_context
        .database
        .get_sticker_set_by_sticker_id(&sticker_id)
        .await?
        .required()?;
    let tags = request_context
        .database
        .get_all_sticker_set_tag_counts(&set.id)
        .await?;

    let r = tags
        .into_iter()
        .skip(current_offset.skip())
        .take(current_offset.page_size())
        .map(|tag| {
            create_query_article(
                request_context.tag_manager.clone(),
                &tag.0,
                &tag.0, // TODO: proper command string
                &format!("{} stickers in this set have this tag", tag.1),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    require_some_results("tags", current_offset, r.len())?;
    request_context
        .bot
        .answer_inline_query(q.id, r.clone())
        .next_offset(current_offset.next_query_offset(r.len()))
        .cache_time(0) // in seconds // TODO: constant?
        .await?;

    Ok(())
}

#[tracing::instrument(skip(q, request_context))]
async fn handle_overlapping_sets(
    current_offset: QueryPage,
    sticker_id: String,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let main_set = request_context
        .database
        .get_sticker_set_by_sticker_id(&sticker_id)
        .await?
        .required()?;
    let set_sticker_count = request_context
        .database
        .get_all_stickers_in_set(&main_set.id)
        .await?
        .len()
        .max(1); // TODO: separate query would probably be more efficient
    let sets = request_context
        .database
        .get_overlapping_sets(&main_set.id)
        .await?;

    let results = sets
        .into_iter()
        .skip(current_offset.skip())
        .take(current_offset.page_size());
    let mut r = Vec::new();
    for (set_id, count) in results {
        // TODO: join in query + pagination in query
        let s = request_context
            .database
            .get_sticker_set_by_id(&set_id)
            .await?
            .required()?;
        r.push(create_query_set(
            &s,
            {
                let p = {
                    let percentage =
                        (((count as f32 / set_sticker_count as f32) * 100.0).round() as i64);
                    format!("{percentage}%")
                };
                Some(if count == 1 {
                    format!("1 overlapping sticker ({p})")
                } else {
                    format!("{count}/{set_sticker_count} overlapping stickers ({p})")
                })
            },
            {
            let (smaller, bigger) = min_max(&set_id, &main_set.id);
            format!( "https://fuzzle-bot.avoonix.com/thumbnails/compare-sticker-sets/{smaller}/{bigger}/image.png")
            }
        )?);
    }

    require_some_results("sets", current_offset, r.len())?;
    request_context
        .bot
        .answer_inline_query(q.id, r.clone())
        .next_offset(current_offset.next_query_offset(r.len()))
        .cache_time(0) // in seconds // TODO: constant?
        .await?;

    Ok(())
}

#[tracing::instrument(skip(q, request_context))]
async fn handle_embedding_query(
    current_offset: QueryPage,
    query: String,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    // TODO: cache?
    // TODO: blacklist?
    let (matches, original_result_len) = find_with_text_embedding(
        request_context.database.clone(),
        query,
        request_context.vector_db,
        request_context.config.clone(),
        current_offset.page_size(),
        current_offset.skip(), 
    )
    .await?;

    let mut stickers = Vec::new();
    for Match {sticker_id, ..} in matches {
        stickers.push(request_context.database.get_sticker_by_id(&sticker_id).await?); // TODO: single query?
    }

    let sticker_result = stickers
        .into_iter()
        .flatten()
        .map(|sticker| {
            InlineQueryResultCachedSticker::new(
                InlineQueryResultId::Sticker(sticker.id).to_string(),
                sticker.telegram_file_identifier,
            )
            .into()
        })
        .collect_vec();

    require_some_results("stickers", current_offset, original_result_len)?;
    request_context
        .bot
        .answer_inline_query(q.id, sticker_result.clone())
        .next_offset(current_offset.next_query_offset(original_result_len))
        .cache_time(0)
        .switch_pm_text(Text::switch_pm_text())
        .switch_pm_parameter(StartParameter::Greeting.to_string())
        .is_personal(true)
        .await?;
    Ok(())
}

#[tracing::instrument(skip(q, request_context))]
async fn handle_most_used_emojis(
    current_offset: QueryPage,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let emojis = request_context
        .database
        .get_most_used_emojis(
            current_offset.page_size() as i64,
            current_offset.skip() as i64,
        )
        .await?;

    let articles = emojis
        .into_iter()
        .enumerate()
        .map(|(index, (emoji, count))| {
            let rank = index + current_offset.skip() + 1;
            let thumbnail_url =
                format!("https://placehold.co/{THUMBNAIL_SIZE}/007f0e/black.png?text={rank}");
            let thumbnail_url = Url::parse(&thumbnail_url)?;
            let emo = emojis::get(&emoji.to_string_with_variant())
                .or_else(|| emojis::get(&emoji.to_string_without_variant()));
            let description = emo.map_or("".to_string(), |emo| emo.name().to_string());
            Ok::<InlineQueryResult, BotError>(
                InlineQueryResultArticle::new(
                    InlineQueryResultId::Emoji(emoji.clone()).to_string(),
                    format!("{} {}", emoji.to_string_with_variant(), description),
                    InputMessageContent::Text(InputMessageContentText::new(Markdown::escaped(
                        emoji.to_string_with_variant(),
                    ))),
                )
                .description(Markdown::escaped(format!("used by {count} stickers")))
                .thumb_url(thumbnail_url)
                .thumb_width(THUMBNAIL_SIZE)
                .thumb_height(THUMBNAIL_SIZE)
                .hide_url(true)
                .into(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    require_some_results("emojis", current_offset, articles.len())?;
    request_context
        .bot
        .answer_inline_query(q.id, articles.clone())
        .next_offset(current_offset.next_query_offset(articles.len()))
        .cache_time(0)
        .switch_pm_text(Text::switch_pm_text())
        .switch_pm_parameter(StartParameter::Greeting.to_string())
        .is_personal(true)
        .await?;
    Ok(())
}

#[tracing::instrument(skip(q, request_context))]
async fn handle_most_duplicated_stickers(
    current_offset: QueryPage,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let stickers = request_context
        .database
        .get_most_duplicated_stickers(
            current_offset.page_size() as i64,
            current_offset.skip() as i64,
        )
        .await?;

    let sticker_result = stickers
        .into_iter()
        .map(|sticker| {
            InlineQueryResultCachedSticker::new(
                InlineQueryResultId::Sticker(sticker.id).to_string(),
                sticker.telegram_file_identifier,
            )
            .into()
        })
        .collect_vec();

    require_some_results("stickers", current_offset, sticker_result.len())?;
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

#[tracing::instrument(skip(q, request_context))]
async fn get_recommended_stickers_in_recommender_mode(
    current_offset: QueryPage,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let (positive_sticker_id, negative_sticker_id) = match request_context.dialog_state() {
        DialogState::Normal | DialogState::ContinuousTag { .. } => {
            return Err(UserError::InvalidMode.into());
        }
        DialogState::StickerRecommender {
            negative_sticker_id,
            positive_sticker_id,
        } => (positive_sticker_id, negative_sticker_id),
        DialogState::TagCreator(tag_creator) => (vec![], vec![]), // TODO: use the example_sticker_id as positive, and don't have any negatives
    };
    let positive_file_ids = request_context
        .database
        .get_sticker_file_ids_by_sticker_id(&positive_sticker_id)
        .await?;
    let mut negative_file_ids = request_context
        .database
        .get_sticker_file_ids_by_sticker_id(&negative_sticker_id)
        .await?;
    let recommended_file_hashes = request_context
        .vector_db
        .find_similar_stickers(
            &positive_file_ids,
            &negative_file_ids,
            SimilarityAspect::Embedding,
            0.0,
            current_offset.page_size() as u64,
            current_offset.skip() as u64,
        )
        .await?
        .required()?;
    let original_result_len = recommended_file_hashes.len();

    let recommended =
        with_sticker_id(request_context.database.clone(), recommended_file_hashes).await?;

    let sticker_ids = recommended.into_iter().map(|m| m.sticker_id).collect_vec();
    let mut stickers = Vec::new();
    for id in sticker_ids {
        if let Some(sticker) = request_context.database.get_sticker_by_id(&id).await? {
            stickers.push(sticker); // TODO: single query?
        }
    }

    let sticker_result = stickers
        .into_iter()
        .map(|sticker| {
            InlineQueryResultCachedSticker::new(
                InlineQueryResultId::Sticker(sticker.id).to_string(),
                sticker.telegram_file_identifier,
            )
            .into()
        })
        .collect_vec();

    require_some_results("stickers", current_offset, original_result_len)?;
    request_context
        .bot
        .answer_inline_query(q.id, sticker_result.clone())
        .next_offset(current_offset.next_query_offset(original_result_len))
        .cache_time(0)
        .switch_pm_text(Text::switch_pm_text())
        .switch_pm_parameter(StartParameter::Greeting.to_string())
        .is_personal(true)
        .await?;
    Ok(())
}

#[tracing::instrument(skip(request_context, q), err(Debug))]
pub async fn handle_tag_creator(
    q: InlineQuery,
    request_context: RequestContext,
    tag_id: String,
    kind: TagKind,
) -> Result<(), BotError> {
    let thumbnail_url = format!("https://fuzzle-bot.avoonix.com/assets/fuzzle_happy.png");
    let thumbnail_url = Url::parse(&thumbnail_url)?;

    let tag_id = create_tag_id(&tag_id);

    let content = InputMessageContent::Text(InputMessageContentText::new(Markdown::escaped(
        format!("/settag {} {tag_id}", kind.to_u8().unwrap_or_default()),
    )));

    let most_similar_tag = request_context.tag_manager.closest_matching_tag(&tag_id);
    let existing_tag = request_context.database.get_tag_by_id(&tag_id).await?;
    if let Some(tag) = existing_tag {
        return Err(UserError::AlreadyExists(if tag.is_pending {
            "pending tag".to_string()
        } else {
            "tag".to_string()
        })
        .into());
    }

    let tag_creator_message = InlineQueryResultArticle::new(
        InlineQueryResultId::Other("tagcreator".to_string()).to_string(),
        match kind {
            TagKind::Main => "Set the main tag name",
            TagKind::Alias => "Add alias",
        },
        content.clone(),
    )
    .description(tag_id)
    .thumb_url(thumbnail_url)
    .thumb_width(THUMBNAIL_SIZE)
    .thumb_height(THUMBNAIL_SIZE)
    .hide_url(true)
    .into();

    let mut articles = vec![tag_creator_message];

    if let Some(most_similar_tag) = most_similar_tag {
        let url = Url::parse(&format!(
            "https://fuzzle-bot.avoonix.com/assets/fuzzle_info.png"
        ))?;

        let info_message = InlineQueryResultArticle::new(
            InlineQueryResultId::Other("tagcreatorinfo".to_string()).to_string(),
            format!("Similar tag already exists"),
            content,
        )
        .description(most_similar_tag)
        .thumb_url(url)
        .thumb_width(THUMBNAIL_SIZE)
        .thumb_height(THUMBNAIL_SIZE)
        .hide_url(true)
        .into();
        articles.push(info_message)
    }

    request_context
        .bot
        .answer_inline_query(q.id, articles)
        .next_offset("")
        .cache_time(0)
        .switch_pm_text(Text::switch_pm_text())
        .switch_pm_parameter(StartParameter::Greeting.to_string())
        .is_personal(true)
        .await?;
    Ok(())
}

#[tracing::instrument(skip(q, request_context))]
async fn handle_user_sets(
    current_offset: QueryPage,
    sticker_id: String,
    set_title: Option<String>,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let sets = request_context
        .database
        .get_owned_sticker_sets_by_bot(
            &request_context.config.telegram_bot_username,
            request_context.user.id,
        )
        .await?;
    let sticker_file = request_context
        .database
        .get_sticker_file_by_sticker_id(&sticker_id)
        .await?
        .required()?;

    let filter_term = &set_title.clone().unwrap_or_default().to_lowercase();

    let sets = sets
        .into_iter()
        .filter(|set| {
            set.id.to_lowercase().contains(filter_term)
                || set
                    .title
                    .as_ref()
                    .map_or(false, |title| title.to_lowercase().contains(filter_term))
        })
        .take(49);

    let mut articles = vec![];
    for set in sets {
        request_context
            .process_sticker_set(set.id.clone(), false)
            .await;
        // TODO: use futuresunordered
        let url = format!(
            "https://fuzzle-bot.avoonix.com/thumbnails/sticker-set/{}/image.png",
            &set.id
        );
        let set_title = set.title.clone().unwrap_or(set.id.clone());

        let sticker_in_set = request_context
            .database
            .sticker_set_contains_file(&set.id, &sticker_file.id)
            .await?;

        let content = InputMessageContent::Text(InputMessageContentText::new(Markdown::escaped(
            if let Some(ref sticker_id) = sticker_in_set {
                format!("/removesticker {} {}", &set.id, &sticker_id)
            } else {
                format!("/addsticker {} {}", &set.id, &sticker_id)
            },
        )));

        let mut article = InlineQueryResultArticle::new(
            InlineQueryResultId::Set(set.id.clone()).to_string(),
            set_title,
            content,
        )
        .description(if sticker_in_set.is_some() {
            "❌ Remove from this set"
        } else {
            "✅ Add to this set"
        });
        let thumbnail_url = Url::parse(&url)?;
        article = article
            .thumb_url(thumbnail_url)
            .thumb_width(THUMBNAIL_SIZE)
            .thumb_height(THUMBNAIL_SIZE)
            .hide_url(true);

        articles.push(article.into());
    }

    let set_title_with_fallback = set_title.clone().unwrap_or_else(|| "Stickers".to_string());
    let set_id = create_sticker_set_id(
        &set_title_with_fallback,
        &request_context.config.telegram_bot_username,
    );

    let url = Url::parse(&format!(
        "https://fuzzle-bot.avoonix.com/assets/fuzzle_happy.png"
    ))?;
    let content = InputMessageContent::Text(InputMessageContentText::new(Markdown::escaped(
        format!("/createset {sticker_id} {set_title_with_fallback}"),
    )));

    let info_message = InlineQueryResultArticle::new(
        InlineQueryResultId::Other("createset".to_string()).to_string(),
        format!("Create: {set_title_with_fallback}"),
        content,
    )
    .description(if set_title.is_none() {
        "Create a new set (add text to change the title)"
    } else {
        "Create a new set"
    })
    .thumb_url(url)
    .thumb_width(THUMBNAIL_SIZE)
    .thumb_height(THUMBNAIL_SIZE)
    .hide_url(true)
    .into();

    articles.push(info_message);

    request_context
        .bot
        .answer_inline_query(q.id, articles)
        .next_offset("") // TODO: paginate
        .cache_time(0) // in seconds // TODO: constant?
        .await?;

    Ok(())
}

#[tracing::instrument(skip(q, request_context))]
async fn handle_stickers_by_date(
    current_offset: QueryPage,
    sticker_id: String,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let set = request_context
        .database
        .get_sticker_set_by_sticker_id(&sticker_id)
        .await?
        .required()?;
    let stickers = request_context
        .database
        .get_all_stickers_in_set(&set.id)
        .await?;

    let r = stickers
        .into_iter()
        .sorted_by_key(|s| s.created_at) // TODO: should we use the sticker_file created_at here?
        .rev()
        .chunk_by(|s| format_relative_time(s.created_at))
        .into_iter()
        .flat_map(|(key, chunk)| {
            // TODO: this looks weird in telegram
            let mut res = vec![InlineQueryResultArticle::new(
                InlineQueryResultId::Other(key.clone()).to_string(),
                format!("{}", &key),
                InputMessageContent::Text(InputMessageContentText::new(Markdown::escaped(
                    format!("{}", key),
                ))),
            )
            .into()];
            for sticker in chunk {
                res.push(
                    InlineQueryResultCachedSticker::new(
                        InlineQueryResultId::Sticker(sticker.id).to_string(),
                        sticker.telegram_file_identifier,
                    )
                    .into(),
                );
            }
            res
        })
        .skip(current_offset.skip())
        .take(current_offset.page_size())
        .collect_vec();

    require_some_results("stickers", current_offset, r.len())?;
    request_context
        .bot
        .answer_inline_query(q.id, r.clone())
        .next_offset(current_offset.next_query_offset(r.len()))
        .cache_time(0) // in seconds // TODO: constant?
        .await?;

    Ok(())
}

#[tracing::instrument(skip(q, request_context))]
async fn get_all_sticker_sets_owned_by_a_user(
    current_offset: QueryPage,
    user_id: i64,
    q: InlineQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let sets = request_context
        .database
        .get_owned_sticker_sets(user_id, current_offset.page_size() as i64, current_offset.skip() as i64)
        .await?;

    let mut r = Vec::new();
    for set in sets {
        r.push(create_query_set(
            &set,
            None,
            format!(
                "https://fuzzle-bot.avoonix.com/thumbnails/sticker-set/{}/image.png",
                &set.id
            )
        )?);
    }

    require_some_results("sets", current_offset, r.len())?;
    request_context
        .bot
        .answer_inline_query(q.id, r.clone())
        .next_offset(current_offset.next_query_offset(r.len()))
        .cache_time(0) // in seconds // TODO: constant?
        .await?;

    Ok(())
}
