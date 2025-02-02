use std::{future::IntoFuture, time::Duration};

use itertools::Itertools;
use nom::combinator::eof;
use nom::combinator::map;
use nom::sequence::tuple;
use nom::{character::complete::multispace0, Finish};
use regex::Regex;
use teloxide::types::ChatId;
use teloxide::{
    dispatching::dialogue::GetChatId,
    payloads::{SendMessageSetters, SendPhotoSetters},
    requests::Requester,
    types::{
        ChatShared, InputFile, LinkPreviewOptions, Message, MessageEntityKind, MessageEntityRef,
        ReplyMarkup, Sticker, UsersShared,
    },
    utils::command::{BotCommands, ParseError},
};
use tracing::{info, Instrument};
use url::Url;

use crate::{
    background_tasks::BackgroundTaskExt,
    bot::{
        report_bot_error, report_internal_error, report_internal_error_result, Bot, BotError,
        BotExt, RequestContext, UserError,
    },
    callback::TagOperation,
    database::DialogState,
    sticker::{fetch_sticker_file, import_individual_sticker_and_queue_set},
    tags::suggest_tags,
    text::{Markdown, Text},
    util::{parse_emoji, Emoji, Required},
};

use super::send_database_export_to_chat;
use super::{
    command::{fix_underline_command_separator_and_normalize, AdminCommand, HiddenCommand, RegularCommand},
    send_sticker_with_tag_input, Keyboard,
};

const fn is_unknown_command(err: &ParseError) -> bool {
    matches!(err, ParseError::UnknownCommand(_))
}

fn get_all_entities_from_message(msg: &Message) -> Vec<MessageEntityRef<'_>> {
    msg.parse_caption_entities()
        .unwrap_or_default()
        .into_iter()
        .chain(msg.parse_entities().unwrap_or_default())
        .collect_vec()
}

fn get_all_urls_from_entities(entities: Vec<MessageEntityRef<'_>>) -> Vec<Url> {
    entities
        .into_iter()
        .filter_map(|entity| match entity.kind() {
            MessageEntityKind::Url => Url::parse(entity.text())
                .or_else(|_| Url::parse(&format!("https://{}", entity.text())))
                .ok(),
            MessageEntityKind::TextLink { url } => Some(url.clone()),
            _ => None,
        })
        .collect_vec()
}

fn get_sticker_set_name_from_url(url: &Url) -> Option<String> {
    match url.host_str() {
        Some("t.me") => {
            let path = url.path();
            path.starts_with("/addstickers/").then(|| {
                path.trim_start_matches("/addstickers/")
                    .trim_end_matches('/')
                    .to_string()
            })
        }
        _ => None,
    }
}

fn find_sticker_set_urls(msg: &Message) -> Vec<String> {
    let entities = get_all_entities_from_message(msg);
    let urls = get_all_urls_from_entities(entities);
    urls.iter()
        .filter_map(get_sticker_set_name_from_url)
        .collect_vec()
}

async fn handle_sticker_sets(
    msg: &Message,
    potential_sticker_set_names: Vec<String>,
    request_context: RequestContext,
) -> Result<(), BotError> {
    if potential_sticker_set_names.len() == 1 {
        let sticker = request_context
            .database
            .get_all_stickers_in_set(&potential_sticker_set_names[0])
            .await?
            .into_iter()
            .next();
        if let Some(sticker) = sticker {
            // message only contains a single set which already exists, show the first sticker
            send_sticker_with_tag_input(sticker, request_context, msg.chat.id, msg.id).await?;
            return Ok(());
        }
    }
    for set_name in &potential_sticker_set_names {
        // TODO: this causes potential loss of sticker packs if bot is restarted before queue is empty
        request_context.importer.queue_sticker_set_import(set_name, false, Some(request_context.user_id())).await;
    }
    request_context
        .bot
        .send_markdown(
            msg.chat.id,
            Text::get_processed_sticker_sets_text(potential_sticker_set_names),
        )
        .reply_to_message_id(msg.id)
        .await?;
    Ok(())
}

fn emoji_search_command(input: &str) -> Option<Emoji> {
    Finish::finish(map(
        tuple((multispace0, parse_emoji, multispace0, eof)),
        |(_, emoji, _, _)| emoji,
    )(input))
    .map(|(_, emoji)| emoji)
    .ok()
}

async fn emoji_search_command_execute(
    msg: &Message,
    request_context: &RequestContext,
    emoji: &Emoji,
) -> Result<(), BotError> {
    request_context
        .bot
        .send_markdown(msg.chat.id, Markdown::escaped(format!("You sent a {} emoji", emoji.to_string_with_variant())))
        .reply_markup(Keyboard::emoji_article(emoji))
        .await?;
    Ok(())
}

#[tracing::instrument(skip(request_context, msg))]
async fn handle_text_message(
    text: &str,
    request_context: RequestContext,
    msg: Message,
) -> Result<(), BotError> {
    let text = &fix_underline_command_separator_and_normalize(text);

    if let Some(emoji) = emoji_search_command(text) {
        return emoji_search_command_execute(&msg, &request_context, &emoji).await;
    }

    if !text.starts_with("/") {
        return Ok(())
    }

    match RegularCommand::parse(text, &request_context.config.telegram_bot_username) {
        Ok(command) => {
            return command.execute(msg, request_context).await;
        }
        Err(err) => {
            if !is_unknown_command(&err) {
                return Err(UserError::CommandError(err).into());
            }
            // ignore unknown command errors
        }
    }

    let err = match HiddenCommand::parse(text, &request_context.config.telegram_bot_username) {
        Ok(command) => {
            return command.execute(msg, request_context).await;
        }
        Err(err) => err,
    };

    if request_context.is_admin() {
        if is_unknown_command(&err) {
            match AdminCommand::parse(text, &request_context.config.telegram_bot_username) {
                Ok(command) => command.execute(msg, request_context).await,
                Err(err) => Err(UserError::CommandError(err).into()),
            }
        } else {
            Err(UserError::CommandError(err).into())
        }
    } else {
        Err(UserError::CommandError(err).into())
    }
}

#[tracing::instrument(skip(request_context, msg))]
pub async fn message_handler_wrapper(
    msg: Message,
    request_context: RequestContext,
) -> Result<(), ()> {
    match message_handler(msg.clone(), request_context.clone()).await {
        Ok(_) => {}
        Err(error) => {
            report_bot_error(&error);
            report_internal_error_result(show_error(msg, request_context, error).await);
        }
    }
    Ok(())
}

#[tracing::instrument(skip(request_context, msg), err(Debug))]
pub async fn show_error(
    msg: Message,
    request_context: RequestContext,
    error: BotError,
) -> Result<(), BotError> {
    let error = error.end_user_error();
    let icon = match error.1 {
        crate::bot::UserErrorSeverity::Error => "⚠️",
        crate::bot::UserErrorSeverity::Info => "ℹ️",
    };
    request_context
        .bot
        .send_markdown(
            msg.chat.id,
            Markdown::escaped(format!("{icon} {}", error.0)),
        )
        .reply_to_message_id(msg.id)
        .allow_sending_without_reply(true)
        .disable_notification(false)
        .await?;
    Ok(())
}

#[tracing::instrument(skip(request_context, msg), err(Debug))]
pub async fn message_handler(
    msg: Message,
    request_context: RequestContext,
) -> Result<(), BotError> {
    for user in msg.mentioned_users() {
        if let Some(username) = &user.username {
            request_context.database.add_username_details(username, crate::database::UsernameKind::User, user.id.0 as i64).await?;
        }
    }
    if let Some (forward) = msg.forward_from_chat() {
        let username = match forward.kind.clone() {
            teloxide::types::ChatKind::Public(chat) => {
                match chat.kind {
                    teloxide::types::PublicChatKind::Channel(channel) => channel.username,
                    teloxide::types::PublicChatKind::Group(group) => None,
                    teloxide::types::PublicChatKind::Supergroup(supergroup) => supergroup.username,
                }
            },
            teloxide::types::ChatKind::Private(chat) => chat.username,
        };

        if let Some(username) = username {
            request_context.database.add_username_details(&username, crate::database::UsernameKind::Channel, forward.id.0 as i64).await?;
        }
    }
    for entity in get_all_entities_from_message(&msg) {
        let urls = get_all_urls_from_entities(vec![entity.clone()]);
        for url in urls {
            match url.host_str() {
                Some("t.me") | Some("telegram.me") => {
                    let path = url.path();
                    let path = path.trim_start_matches("/")
                        .trim_end_matches('/');
                    let matched = Regex::new(r"^[_a-zA-Z0-9]+$").expect("hardcoded url to compile").is_match(path);
                    if matched {
                        request_context.database.add_username(path).await?;
                    }
                }
                _ => {},
            }
        }
        match entity.kind() {
            MessageEntityKind::Mention => {
                let text = entity.text();
                if text.starts_with("@") {
                    let text = text.trim_start_matches("@");
                    let matched = Regex::new(r"^[_a-zA-Z0-9]+$").expect("hardcoded url to compile").is_match(text);
                    if matched {
                        request_context.database.add_username(text).await?;
                    }
                }
            },
            _ => {}
        }
    }
    if request_context.config.is_readonly {
        if let Some(sticker) = msg.sticker() {
            if let Some(ref set_id) = sticker.set_name {
                request_context
                    .database
                    .upsert_sticker_set(set_id, request_context.user.id)
                    .await?;
                return Ok(());
            }
        }
        return send_readonly_message(msg.chat.id, request_context).await;
    }
    let potential_sticker_set_names = find_sticker_set_urls(&msg);
    if !potential_sticker_set_names.is_empty() {
        handle_sticker_sets(&msg, potential_sticker_set_names, request_context).await
    } else if let Some(text) = msg.text() {
        handle_text_message(text, request_context, msg.clone()).await
    } else if let Some(sticker) = msg.sticker() {
        handle_sticker_message(sticker, request_context, msg.clone()).await
    } else if let Some(shared_chat) = msg.shared_chat() {
        if let Some(username) = &shared_chat.username {
            request_context.database.add_username_details(&username, crate::database::UsernameKind::Channel, shared_chat.chat_id.0 as i64).await?;
        }
        handle_shared_chat(&request_context, &msg, shared_chat).await
    } else if let Some(shared_users) = msg.shared_users() {
        for user in &shared_users.users {
            if let Some(username) = &user.username {
                request_context.database.add_username_details(username, crate::database::UsernameKind::User, user.user_id.0 as i64).await?;
            }
        }
        handle_shared_users(&request_context, &msg, shared_users).await
    } else {
        Ok(())
    }
}

pub async fn send_readonly_message(id: ChatId, request_context: RequestContext) -> Result<(), BotError> {
    request_context
        .bot
        .send_markdown(
            id,
            Markdown::escaped("Hi, I am temporarily(?) semi-disabled (you can still search for stickers) until Avoo has more capacity to develop and moderate. 

Thanks to everyone who submitted their sticker packs or helped tagging <3. All packs and taggings can be found below and the source code can be found here https://github.com/avoonix/fuzzle-bot"),
        )
        .link_preview_options(LinkPreviewOptions::new().is_disabled(true))
        .await?;
        send_database_export_to_chat(id, request_context.database, request_context.bot).await?;
        Ok(())
}

#[tracing::instrument(skip(request_context, msg))]
async fn handle_shared_chat(
    request_context: &RequestContext,
    msg: &Message,
    shared_chat: &ChatShared,
) -> Result<(), BotError> {
    let mut state = match request_context.dialog_state() {
        DialogState::TagCreator(state) => state,
        DialogState::ContinuousTag { .. }
        | DialogState::Normal
        | DialogState::StickerRecommender { .. } => {
            return Err(UserError::InvalidMode.into());
        }
    };

    let Some(ref username) = shared_chat.username else {
        return Err(UserError::ChannelWithoutUsername.into());
    };
    state.linked_channel = Some(shared_chat.chat_id.0);
    // TODO: ensure username is saved in database
    let state = state;
    request_context
        .database
        .update_dialog_state(
            request_context.user.id,
            &DialogState::TagCreator(state.clone()),
        )
        .await?;

    request_context
        .bot
        .send_markdown(
            msg.chat.id,
            // TODO: better message
            Markdown::escaped(format!("https://t.me/{}", username)),
        )
        .link_preview_options(LinkPreviewOptions::new().is_disabled(true))
        .reply_markup(Keyboard::tag_creator(&state))
        .await?;
    Ok(())
}

#[tracing::instrument(skip(request_context, msg))]
async fn handle_shared_users(
    request_context: &RequestContext,
    msg: &Message,
    shared_chat: &UsersShared,
) -> Result<(), BotError> {
    let mut state = match request_context.dialog_state() {
        DialogState::TagCreator(state) => state,
        DialogState::ContinuousTag { .. }
        | DialogState::Normal
        | DialogState::StickerRecommender { .. } => {
            return Err(UserError::InvalidMode.into());
        }
    };
    let user = shared_chat.users.first().required()?;
    let Some(ref username) = user.username else {
        return Err(UserError::ChannelWithoutUsername.into());
    };
    state.linked_user = Some(user.user_id.0 as i64);
    // TODO: ensure username is saved in database
    let state = state;
    request_context
        .database
        .update_dialog_state(
            request_context.user.id,
            &DialogState::TagCreator(state.clone()),
        )
        .await?;

    request_context
        .bot
        .send_markdown(
            msg.chat.id,
            // TODO: better message
            Markdown::escaped(format!("https://t.me/{}", username)),
        )
        .link_preview_options(LinkPreviewOptions::new().is_disabled(true))
        .reply_markup(Keyboard::tag_creator(&state))
        .await?;
    Ok(())
}

#[tracing::instrument(skip(request_context, sticker, msg), fields(sticker_id = sticker.file.unique_id, set_id = sticker.set_name))]
async fn handle_sticker_message(
    sticker: &Sticker,
    request_context: RequestContext,
    msg: Message,
) -> Result<(), BotError> {
    // TODO: tell the user how many tags exist for the set/sticker already
    let result =
        import_individual_sticker_and_queue_set(sticker.clone(), request_context.clone()).await?;
    // TODO: notify user that the set can't be added
    // match result {
    //     Err(BotError::Database(crate::database::DatabaseError::TryingToInsertRemovedSet)) => {
    //         request_context
    //             .bot
    //             .send_markdown(msg.chat.id, Text::removed_set())
    //             .reply_markup(Keyboard::removed_set(
    //                 request_context.is_admin(),
    //                 set_name.to_string(),
    //             ))
    //             .allow_sending_without_reply(true)
    //             .reply_to_message_id(msg.id)
    //             .await?;
    //         return Ok(());
    //     }
    //     rest => rest?,
    // };
    // TODO: make return value indicate if the set is new or not -> message admin + tell user that set is new

    match request_context.dialog_state() {
        DialogState::Normal => handle_sticker_1(msg, sticker, request_context, false).await?,
        DialogState::ContinuousTag (continuous_tag) => {
            let file = request_context
                .database
                .get_sticker_file_by_sticker_id(&sticker.file.unique_id)
                .await?
                .required()?;
            request_context
                .database
                .tag_file(&file.id, &continuous_tag.add_tags, Some(request_context.user.id))
                .await?;

            request_context
                .database
                .untag_file(&file.id, &continuous_tag.remove_tags, request_context.user.id)
                .await?;
            request_context.tfidf.request_recompute().await;
            handle_sticker_1(msg, sticker, request_context, true).await?;
        }
        DialogState::StickerRecommender {
            positive_sticker_id,
            negative_sticker_id,
        } => {
            let sticker = request_context
                .database
                .get_sticker_by_id(&sticker.file.unique_id)
                .await?
                .required()?;
            let sticker_user = request_context
                .database
                .get_sticker_user(&sticker.id, request_context.user.id)
                .await?;
            let is_favorited = sticker_user.map_or(false, |su| su.is_favorite); // TODO: duplicated code

            request_context
                .bot
                .send_markdown(msg.chat.id, Markdown::escaped("UwU"))
                .reply_markup(Keyboard::recommender(
                    &sticker.id,
                    &positive_sticker_id,
                    &negative_sticker_id,
                    is_favorited,
                ))
                .allow_sending_without_reply(true)
                .reply_to_message_id(msg.id)
                .into_future()
                .instrument(tracing::info_span!("telegram_bot_send_markdown"))
                .await?;
        }
        DialogState::TagCreator(mut tag_creator) => {
            let sticker = request_context
                .database
                .get_sticker_by_id(&sticker.file.unique_id)
                .await?
                .required()?;
            // let sticker_user = request_context
            //     .database
            //     .get_sticker_user(&sticker.id, request_context.user.id)
            //     .await?;
            // let is_favorited = sticker_user.map_or(false, |su| su.is_favorite); // TODO: duplicated code

            if !tag_creator.example_sticker_id.contains(&sticker.id) {
                tag_creator.example_sticker_id.push(sticker.id.clone());
            }

            request_context
                .database
                .update_dialog_state(
                    request_context.user.id,
                    &DialogState::TagCreator(tag_creator.clone()),
                )
                .await?;

            request_context
                .bot
                .send_markdown(msg.chat.id, Markdown::escaped("UwU")) // TODO: change message
                .reply_markup(Keyboard::tag_creator_sticker(&tag_creator, &sticker))
                .allow_sending_without_reply(true)
                .reply_to_message_id(msg.id)
                .into_future()
                .instrument(tracing::info_span!("telegram_bot_send_markdown"))
                .await?;
        }
    }

    Ok(())
}

#[tracing::instrument(skip(request_context, msg, sticker), fields(sticker_id = sticker.file.unique_id))]
async fn handle_sticker_1(
    // TODO: rename
    msg: Message,
    sticker: &Sticker,
    request_context: RequestContext,
    is_continuous_tag: bool,
) -> Result<(), BotError> {
    let tags: Vec<String> = request_context
        .database
        .get_sticker_tags(&sticker.file.unique_id)
        .await?;
    let set = request_context.database.get_sticker_set_by_sticker_id(&sticker.file.unique_id).await?;
    let suggested_tags = suggest_tags(
        &sticker.file.unique_id,
        request_context.bot.clone(),
        request_context.tag_manager.clone(),
        request_context.database.clone(),
        request_context.tfidf.clone(),
        request_context.vector_db.clone(),
        // request_context.tag_worker.clone(),
    )
    .await?;
    let emojis = sticker
        .emoji
        .clone()
        .map(|e| Emoji::new_from_string_single(e));
    let is_locked = request_context
        .database
        .get_sticker_file_by_sticker_id(&sticker.file.unique_id)
        .await?
        .map_or(false, |file| file.tags_locked_by_user_id.is_some());
    request_context
        .bot
        .send_markdown(
            msg.chat.id,
            if is_continuous_tag {
                Text::continuous_tag_success()
            } else {
                Text::get_sticker_text(emojis, set.is_some_and(|set| set.last_fetched.is_none()))
            },
        )
        .reply_markup(Keyboard::tagging(
            &tags,
            &sticker.file.unique_id.clone(),
            &suggested_tags,
            is_locked,
            is_continuous_tag,
        request_context.tag_manager.clone(),
    )?)
        .allow_sending_without_reply(true)
        .reply_to_message_id(msg.id)
        .into_future()
        .instrument(tracing::info_span!("telegram_bot_send_markdown"))
        .await?;
    Ok(())
}
