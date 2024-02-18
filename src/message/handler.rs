use itertools::Itertools;
use log::info;
use std::sync::Arc;
use teloxide::{
    payloads::SendMessageSetters,
    types::{Me, Message, MessageEntityKind, MessageEntityRef, ReplyMarkup, Sticker},
    utils::command::{BotCommands, ParseError},
};
use url::Url;

use crate::{
    bot::{Bot, BotError, BotExt, Config, UserMeta},
    callback::TagOperation,
    database::Database,
    sticker::import_individual_sticker_and_queue_set,
    tags::{suggest_tags, TagManager},
    text::{Markdown, Text},
    util::Emoji,
    worker::WorkerPool,
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

pub async fn message_handler(
    bot: Bot,
    msg: Message,
    me: Me,
    config: Config,
    tag_manager: Arc<TagManager>,
    worker: WorkerPool,
    database: Database,
    user: UserMeta,
) -> Result<(), BotError> {
    let entities = get_all_entities_from_message(&msg);
    let urls = get_all_urls_from_entities(entities);
    let potential_sticker_set_names = urls
        .iter()
        .filter_map(get_sticker_set_name_from_url)
        .collect_vec();

    let continuous_tag_mode_tag = msg
        .reply_to_message()
        .and_then(Text::parse_continuous_tag_mode_message);

    for set_name in &potential_sticker_set_names {
        worker
            .process_sticker_set(Some(user.id()), set_name.to_string())
            .await;
    }
    let found_sticker_set = !potential_sticker_set_names.is_empty();
    if found_sticker_set {
        bot.send_markdown(
            msg.chat.id,
            Text::get_processed_sticker_sets_text(potential_sticker_set_names),
        )
        .reply_to_message_id(msg.id)
        .await?;
    }
    if let Some(text) = msg.text() {
        if !found_sticker_set {
            let text = &fix_underline_command_separator(text);
            let bot_user = me.username();
            match RegularCommand::parse(text, bot_user) {
                Ok(command) => {
                    command
                        .execute(bot, msg, tag_manager, database, user, config)
                        .await?;
                }
                Err(err) => {
                    if is_unknown_command(&err) {
                        // hidden command
                        match HiddenCommand::parse(text, bot_user) {
                            Ok(command) => {
                                command
                                    .execute(bot, msg, tag_manager, worker, database, user)
                                    .await?;
                            }
                            Err(err) => {
                                if is_unknown_command(&err) && user.is_admin {
                                    // admin command
                                    match AdminCommand::parse(text, bot_user) {
                                        Ok(command) => {
                                            command.execute(bot, msg, database, worker, config).await?;
                                        }
                                        Err(err) => {
                                            handle_command_error(bot, msg, err).await?;
                                        }
                                    }
                                } else {
                                    handle_command_error(bot, msg, err).await?;
                                }
                            }
                        }
                    } else {
                        handle_command_error(bot, msg, err).await?;
                    }
                }
            }
        }
    } else if let Some(sticker) = msg.sticker() {
        if let Some(set_name) = &sticker.set_name {
            handle_sticker(
                bot,
                msg.clone(),
                sticker,
                set_name,
                continuous_tag_mode_tag,
                tag_manager,
                worker,
                database,
                user,
            )
            .await?;
        } else if let Some(tag) = continuous_tag_mode_tag {
            let message = Text::get_continuous_tag_mode_text(
                tag,
                "Sticker must be part of a set to tag it".to_string(),
            );
            bot.send_markdown(msg.chat.id, message) // TODO: do not rely on this text
                .reply_to_message_id(msg.id)
                .reply_markup(ReplyMarkup::ForceReply(
                    teloxide::types::ForceReply::new()
                        .input_field_placeholder(Some("Reply with a sticker".to_string())),
                ))
                .await?;
        } else {
            bot.send_markdown(
                msg.chat.id,
                Markdown::escaped("The sticker must be part of a set"),
            ) // TODO: do not rely on this text
            .reply_to_message_id(msg.id)
            .await?;
        }
    } else if !found_sticker_set {
        bot.send_markdown(
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
    bot: Bot,
    msg: Message,
    sticker: &Sticker,
    set_name: &str,
    continuous_tag_mode_tag: Option<TagOperation>,
    tag_manager: Arc<TagManager>,
    worker: WorkerPool,
    database: Database,
    user: UserMeta,
) -> Result<(), BotError> {
    // TODO: tell the user how many tags exist for the set/sticker already
    import_individual_sticker_and_queue_set(
        sticker.clone(),
        user.id(),
        bot.clone(),
        database.clone(),
        worker.clone(),
    )
    .await?;
    // TODO: make return value indicate if the set is new or not -> message admin + tell user that set is new

    if let Some(tag_operation) = continuous_tag_mode_tag {
        match tag_operation.clone() {
            TagOperation::Tag(tag) => {
                if let Some(implications) = tag_manager.get_implications(&tag) {
                    let tags = implications
                        .into_iter()
                        .chain(std::iter::once(tag.clone()))
                        .collect_vec();
                    database
                        .tag_sticker(sticker.file.unique_id.clone(), tags, Some(user.id().0))
                        .await?;
                }
            }
            TagOperation::Untag(tag) => {
                database
                    .untag_sticker(sticker.file.unique_id.clone(), tag, user.id().0)
                    .await?;
            }
        }
        bot.send_markdown(
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
        let tags: Vec<String> = database
            .get_sticker_tags(sticker.file.unique_id.clone())
            .await?;
        let suggested_tags = suggest_tags(
            &sticker.file.unique_id,
            bot.clone(),
            tag_manager.clone(),
            database.clone(),
        )
        .await?;
        let emojis = Emoji::parse(sticker.emoji.as_ref().unwrap_or(&String::new()));
        let is_locked = database.sticker_is_locked(sticker.file.unique_id.clone()).await?;
        // TODO: get the actual set name
        bot.send_markdown(
            msg.chat.id,
            Text::get_sticker_text(set_name, set_name, &sticker.file.unique_id, emojis),
        )
        .reply_markup(Keyboard::make_tag_keyboard(
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
            dbg!(command);
            "an error occured".to_string()
        }
    };
    bot.send_markdown(msg.chat.id, Markdown::escaped(message))
        .reply_to_message_id(msg.id)
        .allow_sending_without_reply(true)
        .await?;

    Ok(())
}
