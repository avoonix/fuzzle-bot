use itertools::Itertools;
use teloxide::{
    payloads::SendMessageSetters,
    types::{InlineKeyboardMarkup, UserId},
};

use crate::{
    bot::{Bot, BotExt, InternalError},
    database::{Database, ModerationTask},
    message::Keyboard,
    text::{Markdown, Text},
};

pub async fn create_and_send_daily_moderation_tasks(database: Database, bot: Bot, admin_id: UserId) -> Result<(), InternalError> {
    create_moderation_tasks_for_new_sets(&database).await?;
    send_daily_report(database, bot, admin_id).await?;
    Ok(())
}

pub async fn send_daily_report(
    database: Database,
    bot: Bot,
    admin_id: UserId,
) -> Result<(), InternalError> {
    let counts = database.get_stats().await?;
    let stats = database.get_admin_stats().await?;
    let taggings = database.get_user_tagging_stats_24_hours().await?;

    bot.send_markdown(
        admin_id,
        Text::daily_report(counts, stats, taggings.clone()),
    )
    .reply_markup(Keyboard::daily_report(taggings)?)
    .await?;

    send_moderation_tasks(database, bot, admin_id).await?;

    Ok(())
}

pub async fn create_moderation_tasks_for_new_sets(database: &Database) -> Result<(), InternalError> {
    let new_sticker_sets = database.get_sticker_sets_added_24_hours().await?;
    let new_sticker_sets = new_sticker_sets
        .into_iter()
        .sorted_by_key(|(_, user)| user.clone())
        .chunk_by(|(_, user)| user.clone())
        .into_iter()
        .map(|(user_id, chunk)| {
            (
                user_id.map(|user_id| UserId(user_id as u64)),
                chunk.map(|(set_name, _)| set_name).collect_vec(),
            )
        })
        .collect_vec();

    for (user_id, set_names) in new_sticker_sets {
        for set_names in set_names.chunks(10) {
            let Some(user_id) = user_id else {
                tracing::error!("some sets were added without setting added_by_user_id");
                continue;
            };
                database
                .create_moderation_task(
                    &crate::database::ModerationTaskDetails::ReviewNewSets { 
                        set_ids: set_names.into_iter().cloned().collect_vec()
                    },
                    user_id.0 as i64,
                )
                .await?;
        }
    }
    Ok(())
}

pub async fn send_moderation_tasks(
    database: Database,
    bot: Bot,
    admin_id: UserId,
) -> Result<(), InternalError> {
    let moderation_tasks = database.get_open_moderation_tasks().await?;

    for moderation_task in moderation_tasks {
        let (text, keyboard) = get_moderation_task_data(moderation_task, &database).await?;
        bot.send_markdown(admin_id, text)
            .reply_markup(keyboard)
            .await?;
    }

    Ok(())
}

pub async fn get_moderation_task_data(
    moderation_task: ModerationTask,
    database: &Database,
) -> Result<(Markdown, InlineKeyboardMarkup), InternalError> {
    match moderation_task.details {
        crate::database::ModerationTaskDetails::CreateTag {
            tag_id,
            linked_channel,
            linked_user,
            category,
            example_sticker_id, // TODO: use this
            aliases,
            implications,
        } => {
            let user_username = if let Some(linked_user) = linked_user {
                database.get_username(crate::database::UsernameKind::User, linked_user).await?
            } else { None };
            let channel_username = if let Some(linked_channel) = linked_channel {
                database.get_username(crate::database::UsernameKind::Channel, linked_channel).await?
            } else { None };
            let tag = database.get_tag_by_id(&tag_id).await?;
            Ok((
                Text::create_tag_task(
                    &tag_id,
                    category,
                    &aliases,
                    &implications,
                    tag,
                ),
                // todo! add buttons to show example stickers (or inline query)
                Keyboard::create_tag_task(
                    moderation_task.completion_status,
                    moderation_task.created_by_user_id,
                    moderation_task.id,
                    user_username,
                    channel_username,
                ),
            ))
        }
        crate::database::ModerationTaskDetails::ReportStickerSet { set_id, reason } => Ok((
            Text::report_sticker_set_task(),
            Keyboard::report_sticker_set_task(
                moderation_task.completion_status,
                moderation_task.created_by_user_id,
                moderation_task.id,
                &set_id,
                database.get_set_ids_by_set_ids(&[set_id.clone()]).await?.is_empty(),
            )?,
        )),
        crate::database::ModerationTaskDetails::ReviewNewSets {
            set_ids,
        } => Ok((
            Text::review_new_sets_task(),
            Keyboard::review_new_sets_task(
                moderation_task.completion_status,
                moderation_task.created_by_user_id,
                moderation_task.id,
                &set_ids,
                &database.get_set_ids_by_set_ids(&set_ids).await?,
            )?,
        )),
    }
}
