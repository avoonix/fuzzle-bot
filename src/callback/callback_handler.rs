use itertools::Itertools;
use teloxide::payloads::AnswerCallbackQuerySetters;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardMarkup, MediaKind, MessageCommon, MessageKind};

use crate::bot::{Bot, BotError, BotExt, UserMeta};
use crate::callback::TagOperation;
use crate::database::Database;
use crate::message::Keyboard;
use crate::tags::{suggest_tags, TagManager};
use crate::text::{Markdown, Text};
use crate::util::teloxide_error_can_safely_be_ignored;
use crate::worker::WorkerPool;
use std::sync::Arc;

use crate::callback::CallbackData;

async fn handle_sticker_tag_action(
    operation: TagOperation,
    unique_id: String,
    q: CallbackQuery,
    tag_manager: Arc<TagManager>,
    user: UserMeta,
    database: Database,
    bot: Bot,
) -> Result<(), BotError> {
    if !user.can_tag_stickers() {
        return Err(anyhow::anyhow!("user is not permitted to tag stickers"))?;
    }
    let Some(set_name) = database.get_set_name(unique_id.clone()).await? else {
        // TODO: inform admin; this should not happen
        return answer_callback_query(bot, q, Some(Text::sticker_not_found()), None, None).await;
    };

    let notification;

    match operation {
        TagOperation::Tag(tag) => {
            if let Some(implications) = tag_manager.get_implications(&tag) {
                let tags = implications
                    .clone()
                    .into_iter()
                    .chain(std::iter::once(tag.clone()))
                    .collect_vec();
                database
                    .tag_sticker(unique_id.clone(), tags, Some(q.from.id.0))
                    .await?;
                notification = (!implications.is_empty()).then(|| {
                    format!(
                        "{tag} implies {} (added automatically)",
                        implications.join(", ")
                    )
                });
            } else {
                notification = None;
            }
        }
        TagOperation::Untag(tag) => {
            database
                .untag_sticker(unique_id.clone(), tag.clone(), q.from.id.0)
                .await?;
            notification = tag_manager.get_implications(&tag).and_then(|implications| {
                if implications.is_empty() {
                    None
                } else {
                    Some(format!(
                        "{tag} implies {} (implications are not removed automatically)",
                        implications.join(", ")
                    ))
                }
            });
        }
    }

    let tags = database.get_sticker_tags(unique_id.clone()).await?;
    let suggested_tags =
        suggest_tags(&unique_id, bot.clone(), tag_manager.clone(), database).await?;
    let keyboard = Some(Keyboard::make_tag_keyboard(
        &tags,
        &unique_id,
        &suggested_tags,
        Some(set_name.clone()),
    ));

    answer_callback_query(bot, q, None, keyboard, notification).await
}

pub async fn callback_handler(
    bot: Bot,
    q: CallbackQuery,
    tag_manager: Arc<TagManager>,
    worker: WorkerPool,
    database: Database,
    user: UserMeta,
) -> Result<(), BotError> {
    let data: CallbackData = q.data.clone().unwrap_or_default().try_into()?;
    match data {
        CallbackData::Sticker {
            unique_id,
            operation,
        } => {
            handle_sticker_tag_action(operation, unique_id, q, tag_manager, user, database, bot)
                .await
        }
        CallbackData::RemoveBlacklistedTag(tag) => {
            remove_blacklist_tag(database, q, tag, bot).await
        }
        CallbackData::RemoveSet(set_name) => remove_set(set_name, user, database, bot, q).await,
        CallbackData::Help => {
            answer_callback_query(
                bot,
                q,
                Some(Text::get_help_text(user.is_admin)),
                Some(Keyboard::make_help_keyboard()),
                None,
            )
            .await
        }
        CallbackData::Settings => {
            answer_callback_query(
                bot,
                q,
                Some(Text::get_settings_text()),
                Some(Keyboard::make_settings_keyboard()),
                None,
            )
            .await
        }
        CallbackData::Blacklist => {
            let blacklist = database.get_blacklist(q.from.id.0).await?;
            answer_callback_query(
                bot,
                q,
                Some(Text::get_blacklist_text()),
                Some(Keyboard::make_blacklist_keyboard(&blacklist)),
                None,
            )
            .await
        }
        // show main menu: show main menu, edit message, add keyboard
        CallbackData::Start => {
            answer_callback_query(
                bot,
                q,
                Some(Text::get_main_text()),
                Some(Keyboard::make_main_keyboard()),
                None,
            )
            .await
        }
        CallbackData::Info => {
            answer_callback_query(
                bot,
                q,
                Some(Text::get_info_text()),
                Some(Keyboard::make_info_keyboard()),
                None,
            )
            .await
        }
    }
}

async fn remove_blacklist_tag(
    database: Database,
    q: CallbackQuery,
    tag: String,
    bot: Bot,
) -> Result<(), BotError> {
    database
        .remove_blacklisted_tag(q.from.id.0, tag.clone())
        .await?;
    let blacklist = database.get_blacklist(q.from.id.0).await?;
    let keyboard = Keyboard::make_blacklist_keyboard(&blacklist);
    answer_callback_query(
        bot,
        q,
        Some(Text::get_blacklist_text()),
        Some(keyboard),
        None,
    )
    .await?;
    Ok(())
}

async fn remove_set(
    set_name: String,
    user: UserMeta,
    database: Database,
    bot: Bot,
    q: CallbackQuery,
) -> Result<(), BotError> {
    if !user.is_admin {
        return Err(anyhow::anyhow!(
            "user is not permitted to tag stickers (callback handler)"
        ))?;
    }
    // TODO: maybe require a confirmation? or add a "undo" keyboard?
    // TODO: only allow admin
    database.ban_set(set_name).await?;
    answer_callback_query(
        bot,
        q,
        Some(Markdown::escaped("Deleted Set".to_string())),
        None,
        None,
    )
    .await?;
    Ok(())
}

/// answers the user by editing the message
async fn answer_callback_query(
    bot: Bot,
    q: CallbackQuery,
    text: Option<Markdown>,
    keyboard: Option<InlineKeyboardMarkup>,
    notification: Option<String>,
) -> Result<(), BotError> {
    if let Some(keyboard) = keyboard {
        if let Some(Message { id, chat, kind, .. }) = q.message {
            if let Some(text) = text {
                // if the message is a sticker
                if let MessageKind::Common(MessageCommon {
                    media_kind: MediaKind::Sticker(_sticker),
                    ..
                }) = kind
                {
                    // bot.edit_message_media(chat.id, id, sticker).reply_markup(keyboard).await?;
                    bot.edit_message_reply_markup(chat.id, id)
                        .reply_markup(keyboard)
                        .await?;
                } else {
                    bot.edit_message_markdown(chat.id, id, text)
                        .reply_markup(keyboard)
                        .await?;
                }
            } else {
                bot.edit_message_reply_markup(chat.id, id)
                    .reply_markup(keyboard)
                    .await?;
            }
            // } else if let Some(id) = q.inline_message_id { // TODO: remove, as I don't send inline
            // messages with buttons
            //     bot.edit_message_text_inline(id, text).reply_markup(keyboard).await?;
        }
    }

    let result = match notification {
        Some(text) => {
            bot.answer_callback_query(&q.id)
                .text(text)
                .show_alert(false)
                .await
        }
        None => bot.answer_callback_query(&q.id).await,
    };
    match result {
        Ok(_) => Ok(()),
        Err(err) => {
            if teloxide_error_can_safely_be_ignored(&err) {
                Ok(())
            } else {
                Err(err.into())
            }
        }
    }
}