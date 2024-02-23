use itertools::Itertools;
use log::info;

use teloxide::{
    payloads::SendMessageSetters,
    types::{Message, MessageEntityKind, MessageEntityRef, ReplyMarkup, Sticker},
    utils::command::{BotCommands, ParseError},
};
use url::Url;

use crate::{
    background_tasks::BackgroundTaskExt,
    bot::{Bot, BotError, BotExt, RequestContext},
    callback::TagOperation,
    sticker::import_individual_sticker_and_queue_set,
    tags::{suggest_tags},
    text::{Markdown, Text},
    util::{Emoji},
};

use super::{
    command::{fix_underline_command_separator, AdminCommand, HiddenCommand, RegularCommand},
    Keyboard,
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
    for set_name in &potential_sticker_set_names {
        request_context
            .process_sticker_set(set_name.to_string())
            .await;
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

async fn handle_text_message(
    text: &str,
    request_context: RequestContext,
    msg: Message,
) -> Result<(), BotError> {
    let text = &fix_underline_command_separator(text);
    match RegularCommand::parse(text, &request_context.config.telegram.username) {
        Ok(command) => {
            command.execute(msg, request_context).await?;
        }
        Err(err) => {
            if is_unknown_command(&err) {
                match HiddenCommand::parse(text, &request_context.config.telegram.username) {
                    Ok(command) => {
                        command.execute(msg, request_context).await?;
                    }
                    Err(err) => {
                        if is_unknown_command(&err) && request_context.is_admin() {
                            match AdminCommand::parse(
                                text,
                                &request_context.config.telegram.username,
                            ) {
                                Ok(command) => {
                                    command.execute(msg, request_context).await?;
                                }
                                Err(err) => {
                                    handle_command_error(request_context.bot, msg, err).await?;
                                }
                            }
                        } else {
                            handle_command_error(request_context.bot, msg, err).await?;
                        }
                    }
                }
            } else {
                handle_command_error(request_context.bot, msg, err).await?;
            }
        }
    }
    Ok(())
}

async fn handle_sticker_message(
    sticker: &Sticker,
    request_context: RequestContext,
    msg: Message,
    continuous_tag_mode_tag: Option<TagOperation>,
) -> Result<(), BotError> {
    if let Some(set_name) = &sticker.set_name {
        handle_sticker(
            msg.clone(),
            sticker,
            set_name,
            continuous_tag_mode_tag,
            request_context.clone(),
        )
        .await?;
    } else if let Some(tag) = continuous_tag_mode_tag {
        let message = Text::get_continuous_tag_mode_text(
            tag,
            "Sticker must be part of a set to tag it".to_string(),
        );
        request_context
            .bot
            .send_markdown(msg.chat.id, message) // TODO: do not rely on this text
            .reply_to_message_id(msg.id)
            .reply_markup(ReplyMarkup::ForceReply(
                teloxide::types::ForceReply::new()
                    .input_field_placeholder(Some("Reply with a sticker".to_string())),
            ))
            .await?;
    } else {
        request_context
            .bot
            .send_markdown(
                msg.chat.id,
                Markdown::escaped("The sticker must be part of a set"),
            ) // TODO: do not rely on this text
            .reply_to_message_id(msg.id)
            .await?;
    }
    Ok(())
}

pub async fn message_handler(
    msg: Message,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let potential_sticker_set_names = find_sticker_set_urls(&msg);
    if !potential_sticker_set_names.is_empty() {
        return handle_sticker_sets(&msg, potential_sticker_set_names, request_context).await;
    }

    let continuous_tag_mode_tag = msg
        .reply_to_message()
        .and_then(Text::parse_continuous_tag_mode_message);

    if let Some(text) = msg.text() {
        return handle_text_message(text, request_context, msg.clone()).await;
    } else if let Some(sticker) = msg.sticker() {
        return handle_sticker_message(
            sticker,
            request_context,
            msg.clone(),
            continuous_tag_mode_tag,
        )
        .await;
    } else {
        request_context.bot.send_markdown(
            msg.chat.id,
            Markdown::escaped("I have no idea what to do with this. Send me text messages containing commands, stickers, or sticker set links!"),
        )
        .reply_to_message_id(msg.id)
        .await?;
        info!("not sure what to do with this message: {:?}", msg);
    }

    Ok(())
}

async fn handle_sticker(
    msg: Message,
    sticker: &Sticker,
    set_name: &str,
    continuous_tag_mode_tag: Option<TagOperation>,
    request_context: RequestContext,
) -> Result<(), BotError> {
    // TODO: tell the user how many tags exist for the set/sticker already
    let result =
        import_individual_sticker_and_queue_set(sticker.clone(), request_context.clone()).await;
    match result {
        Err(BotError::Database(crate::database::DatabaseError::TryingToInsertRemovedSet)) => {
            request_context
                .bot
                .send_markdown(msg.chat.id, Text::removed_set())
                .reply_markup(Keyboard::removed_set(
                    request_context.is_admin(),
                    set_name.to_string(),
                ))
                .allow_sending_without_reply(true)
                .reply_to_message_id(msg.id)
                .await?;
            return Ok(());
        }
        rest => rest?,
    };
    // TODO: make return value indicate if the set is new or not -> message admin + tell user that set is new

    if let Some(tag_operation) = continuous_tag_mode_tag {
        match tag_operation.clone() {
            TagOperation::Tag(tag) => {
                if let Some(implications) = request_context.tag_manager.get_implications(&tag) {
                    let tags = implications
                        .into_iter()
                        .chain(std::iter::once(tag.clone()))
                        .collect_vec();
                    request_context
                        .database
                        .tag_sticker(
                            sticker.file.unique_id.clone(),
                            tags,
                            Some(request_context.user_id().0),
                        )
                        .await?;
                    request_context.tagging_worker.maybe_recompute().await?;
                }
            }
            TagOperation::Untag(tag) => {
                request_context
                    .database
                    .untag_sticker(
                        sticker.file.unique_id.clone(),
                        tag,
                        request_context.user_id().0,
                    )
                    .await?;
            }
        }
        request_context
            .bot
            .send_markdown(
                msg.chat.id,
                Text::get_continuous_tag_mode_text(
                    tag_operation,
                    "Sticker processed\\. Send the next one\\.".to_string(),
                ),
            )
            // TODO: reply markup: undo
            // .reply_markup(make_tag_keyboard(&tags, sticker.file.unique_id.clone(), &suggested_tags, set_name.to_string()))
            .allow_sending_without_reply(true)
            .reply_to_message_id(msg.id)
            .reply_markup(ReplyMarkup::ForceReply(
                teloxide::types::ForceReply::new()
                    .input_field_placeholder(Some("Reply with a sticker".to_string())),
            ))
            .await?;
    } else {
        let tags: Vec<String> = request_context
            .database
            .get_sticker_tags(sticker.file.unique_id.clone())
            .await?;
        let suggested_tags = suggest_tags(
            &sticker.file.unique_id,
            request_context.bot.clone(),
            request_context.tag_manager.clone(),
            request_context.database.clone(),
            request_context.tagging_worker.clone(),
        )
        .await?;
        let emojis = Emoji::parse(sticker.emoji.as_ref().unwrap_or(&String::new()));
        let is_locked = request_context
            .database
            .sticker_is_locked(sticker.file.unique_id.clone())
            .await?;
        // TODO: get the actual set name
        request_context
            .bot
            .send_markdown(
                msg.chat.id,
                Text::get_sticker_text(set_name, set_name, &sticker.file.unique_id, emojis),
            )
            .reply_markup(Keyboard::tagging(
                &tags,
                &sticker.file.unique_id.clone(),
                &suggested_tags,
                Some(set_name.to_string()),
                is_locked,
            ))
            .allow_sending_without_reply(true)
            .reply_to_message_id(msg.id)
            .await?;
    }
    Ok(())
}

pub async fn handle_command_error(bot: Bot, msg: Message, err: ParseError) -> Result<(), BotError> {
    let message = match err {
        ParseError::UnknownCommand(command) => {
            format!("unknown command {command}")
        }
        command => {
            // TODO: handle more kinds of errors
            drop(command);
            "an error occured".to_string()
        }
    };
    bot.send_markdown(msg.chat.id, Markdown::escaped(message))
        .reply_to_message_id(msg.id)
        .allow_sending_without_reply(true)
        .await?;

    Ok(())
}
