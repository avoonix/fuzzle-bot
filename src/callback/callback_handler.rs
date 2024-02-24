use itertools::Itertools;

use teloxide::payloads::AnswerCallbackQuerySetters;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardMarkup, MediaKind, MessageCommon, MessageKind};

use crate::bot::{BotError, BotExt, RequestContext};
use crate::callback::TagOperation;

use crate::message::Keyboard;
use crate::sticker::import_all_stickers_from_set;
use crate::tags::suggest_tags;
use crate::text::{Markdown, Text};
use crate::util::teloxide_error_can_safely_be_ignored;

use crate::callback::CallbackData;

async fn change_sticker_locked_status(
    lock: bool,
    unique_id: String,
    q: CallbackQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    if !request_context.user.can_tag_stickers() {
        return Err(anyhow::anyhow!(
            "user is not permitted to change locked status"
        ))?;
    }
    let Some(set_name) = request_context
        .database
        .get_set_name(unique_id.clone())
        .await?
    else {
        // TODO: inform admin; this should not happen
        return answer_callback_query(
            request_context.clone(),
            q,
            Some(Text::sticker_not_found()),
            None,
            None,
        )
        .await;
    };
    request_context
        .database
        .set_locked(unique_id.clone(), request_context.user.id().0, lock)
        .await?;

    send_tagging_keyboard(request_context.clone(), set_name, None, unique_id, q).await
}

async fn handle_sticker_tag_action(
    operation: TagOperation,
    unique_id: String,
    q: CallbackQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    if !request_context.user.can_tag_stickers() {
        return Err(anyhow::anyhow!("user is not permitted to tag stickers"))?;
    }
    let Some(set_name) = request_context
        .database
        .get_set_name(unique_id.clone())
        .await?
    else {
        // TODO: inform admin; this should not happen
        return answer_callback_query(
            request_context.clone(),
            q,
            Some(Text::sticker_not_found()),
            None,
            None,
        )
        .await;
    };

    let notification;

    match operation {
        TagOperation::Tag(tag) => {
            if let Some(implications) = request_context.tag_manager.get_implications(&tag) {
                let tags = implications
                    .clone()
                    .into_iter()
                    .chain(std::iter::once(tag.clone()))
                    .collect_vec();
                request_context
                    .database
                    .tag_sticker(unique_id.clone(), tags, Some(request_context.user_id().0))
                    .await?;
                request_context.tagging_worker.maybe_recompute().await?;
                notification = if implications.is_empty() {
                    "Saved!".to_string()
                } else {
                    format!("Saved! ({tag} implies {})", implications.join(", "))
                };
            } else {
                notification = "Invalid tag!".to_string();
            }
        }
        TagOperation::Untag(tag) => {
            request_context
                .database
                .untag_sticker(unique_id.clone(), tag.clone(), request_context.user_id().0)
                .await?;
            let implications = request_context.tag_manager.get_implications(&tag);
            notification = implications.map_or_else(
                || "Tag does not exist!".to_string(),
                |implications| {
                    if implications.is_empty() {
                        "Removed!".to_string()
                    } else {
                        format!(
                            "Removed just this tag! ({tag} implies {})",
                            implications.join(", ")
                        )
                    }
                },
            );
        }
    }

    send_tagging_keyboard(request_context, set_name, Some(notification), unique_id, q).await
}

async fn send_tagging_keyboard(
    request_context: RequestContext,
    set_name: String,
    notification: Option<String>,
    unique_id: String,
    q: CallbackQuery,
) -> Result<(), BotError> {
    // database: Database,
    // tag_manager: Arc<TagManager>,
    // bot: Bot,
    let tags = request_context
        .database
        .get_sticker_tags(unique_id.clone())
        .await?;
    let suggested_tags = suggest_tags(
        &unique_id,
        request_context.bot.clone(),
        request_context.tag_manager.clone(),
        request_context.database.clone(),
        request_context.tagging_worker.clone(),
    )
    .await?;
    let is_locked = request_context
        .database
        .sticker_is_locked(unique_id.clone())
        .await?;
    let keyboard = Some(Keyboard::tagging(
        &tags,
        &unique_id,
        &suggested_tags,
        Some(set_name.clone()),
        is_locked,
    ));

    answer_callback_query(request_context, q, None, keyboard, notification).await
}

pub async fn callback_handler(
    q: CallbackQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    // bot: Bot,
    // tag_manager: Arc<TagManager>,
    // database: Database,
    // user: UserMeta,
    let data: CallbackData = q.data.clone().unwrap_or_default().try_into()?;
    match data {
        CallbackData::SetLock { lock, sticker_id } => {
            change_sticker_locked_status(lock, sticker_id, q, request_context).await
        }
        CallbackData::Sticker {
            unique_id,
            operation,
        } => handle_sticker_tag_action(operation, unique_id, q, request_context).await,
        CallbackData::RemoveBlacklistedTag(tag) => {
            remove_blacklist_tag(q, tag, request_context).await
        }
        CallbackData::ChangeSetStatus { set_name, banned } => {
            if banned {
                ban_set(set_name, q, request_context).await
            } else {
                unban_set(set_name, q, request_context).await
            }
        }
        CallbackData::Help => {
            answer_callback_query(
                request_context.clone(),
                q,
                Some(Text::get_help_text(request_context.is_admin())),
                Some(Keyboard::make_help_keyboard()),
                None,
            )
            .await
        }
        CallbackData::Settings => {
            answer_callback_query(
                request_context.clone(),
                q,
                Some(Text::get_settings_text(
                    request_context.user.user.settings.clone(),
                )),
                Some(Keyboard::make_settings_keyboard(
                    request_context.user.user.settings.clone(),
                )),
                None,
            )
            .await
        }
        CallbackData::Blacklist => {
            answer_callback_query(
                request_context.clone(),
                q,
                Some(Text::blacklist()),
                Some(Keyboard::blacklist(&request_context.user.user.blacklist)),
                None,
            )
            .await
        }
        // show main menu: show main menu, edit message, add keyboard
        CallbackData::Start => {
            answer_callback_query(
                request_context,
                q,
                Some(Text::get_main_text()),
                Some(Keyboard::make_main_keyboard()),
                None,
            )
            .await
        }
        CallbackData::Info => {
            answer_callback_query(
                request_context,
                q,
                Some(Text::infos()),
                Some(Keyboard::make_info_keyboard()),
                None,
            )
            .await
        }
        CallbackData::UserInfo(user_id) => {
            if !request_context.user.is_admin {
                return Ok(());
            }
            request_context.bot.answer_callback_query(&q.id).await?;

            let user_stats = request_context.database.get_user_stats(user_id).await?;
            // TODO: allow each user to view their own stats

            request_context
                .bot
                .send_markdown(
                    request_context.user_id(),
                    Text::user_stats(user_stats, user_id),
                )
                .reply_markup(Keyboard::user_stats(user_id)?)
                .await?;
            Ok(())
        }
        CallbackData::SetOrder(order) => {
            let mut settings = request_context.user.user.settings.clone();
            settings.order = Some(order);
            request_context
                .database
                .update_settings(request_context.user_id().0, settings.clone())
                .await?;
            answer_callback_query(
                request_context,
                q,
                Some(Text::get_settings_text(settings.clone())),
                Some(Keyboard::make_settings_keyboard(settings)),
                None,
            )
            .await
        }
    }
}

async fn remove_blacklist_tag(
    q: CallbackQuery,
    tag: String,
    request_context: RequestContext,
) -> Result<(), BotError> {
    request_context
        .database
        .remove_blacklisted_tag(request_context.user_id().0, tag.clone())
        .await?;
    let user = request_context
        .database
        .get_user(request_context.user_id().0)
        .await?
        .ok_or(anyhow::anyhow!("user does not exist (should never happen)"))?;
    let keyboard = Keyboard::blacklist(&user.blacklist);
    answer_callback_query(
        request_context,
        q,
        Some(Text::blacklist()),
        Some(keyboard),
        None,
    )
    .await?;
    Ok(())
}

async fn unban_set(
    set_name: String,
    q: CallbackQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    if !request_context.is_admin() {
        return Err(anyhow::anyhow!("user is not permitted to unban sets"))?;
    }
    request_context.database.unban_set(set_name.clone()).await?;
    import_all_stickers_from_set(
        set_name.clone(),
        true,
        request_context.bot.clone(),
        request_context.database.clone(),
    )
    .await?;
    answer_callback_query(
        request_context,
        q,
        Some(Markdown::escaped(format!("Added Set {set_name}"))),
        Some(InlineKeyboardMarkup::new([[]])), // TODO: refactor
        None,
    )
    .await?;
    Ok(())
}

async fn ban_set(
    set_name: String,
    q: CallbackQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    if !request_context.is_admin() {
        return Err(anyhow::anyhow!("user is not permitted to remove sets"))?;
    }
    // TODO: maybe require a confirmation? or add a "undo" keyboard?
    request_context.database.ban_set(set_name.clone()).await?;
    answer_callback_query(
        request_context.clone(),
        q,
        Some(Text::removed_set()),
        Some(Keyboard::removed_set(request_context.is_admin(), set_name)),
        None,
    )
    .await?;
    Ok(())
}

/// answers the user by editing the message
async fn answer_callback_query(
    request_context: RequestContext,
    q: CallbackQuery,
    text: Option<Markdown>,
    keyboard: Option<InlineKeyboardMarkup>,
    notification: Option<String>,
) -> Result<(), BotError> {
    let bot = request_context.bot;
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
        Err(err) if teloxide_error_can_safely_be_ignored(&err) => Ok(()),
        Err(err) => Err(err.into()),
    }
}
