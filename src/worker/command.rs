use log::info;
use teloxide::payloads::SendMessageSetters;
use teloxide::types::InlineKeyboardButton;
use teloxide::types::UserId;
use url::Url;

use crate::bot::Bot;
use crate::bot::BotError;
use crate::bot::BotExt;
use crate::callback::CallbackData;
use crate::database::Database;
use crate::message::send_database_export_to_chat;
use crate::sticker::import_all_stickers_from_set;
use crate::text::Markdown;
use std::fmt::Write;

use super::WorkerPool;

#[derive(Debug)]
pub(super) enum Command {
    SendMessageToAdmin {
        source_user: UserId,
        msg: AdminMessage,
    },
    ProcessStickerSet {
        source_user: Option<UserId>,
        set_name: String,
    },
    ProcessSetOfSticker {
        sticker_unique_id: String,
        source_user: Option<UserId>,
    },
    RefetchAllSets,
    RefetchScheduled {
        count: i64,
    },
    SendReport,
    SendExport,
}

#[derive(Debug)]
pub enum AdminMessage {
    NewUser,
    StickerSetAdded { set_name: String },
}

async fn send_message_to_admin(
    msg: AdminMessage,
    source_user: UserId,
    bot: Bot,
    admin_id: UserId,
) -> Result<(), BotError> {
    let mut keyboard = teloxide::types::InlineKeyboardMarkup::default().append_row(vec![
        InlineKeyboardButton::url(
            "Show User",
            Url::parse(format!("tg://user?id={}", source_user.0).as_str())?,
        ),
    ]);

    let msg = match msg {
        AdminMessage::StickerSetAdded { set_name } => {
            keyboard = keyboard
                .append_row(vec![InlineKeyboardButton::url(
                    "Show Set",
                    Url::parse(format!("https://t.me/addstickers/{}", &set_name).as_str())?,
                )])
                .append_row(vec![InlineKeyboardButton::callback(
                    "Delete/Ban Set",
                    CallbackData::remove_set(&set_name).to_string(),
                )]);
            format!("Added Set {set_name} via URL") // TODO: pretty markdown formatting
        }
        msg => {
            format!("Message: {msg:?}")
        }
    };

    bot.send_markdown(admin_id, Markdown::escaped(msg))
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

async fn process_set_of_sticker(
    sticker_unique_id: String,
    source_user: Option<UserId>,
    database: Database,
    bot: Bot,
    worker: WorkerPool,
) -> Result<(), BotError> {
    // TODO: retry on error
    let set_name = database.get_set_name(sticker_unique_id.clone()).await?;
    match set_name {
        Some(set_name) => {
            // let result = fetch_sticker_set(bot.clone(), set_name, None, database.clone(), false, worker.clone()).await;
            import_all_stickers_from_set(
                set_name,
                source_user,
                false,
                bot.clone(),
                database.clone(),
                worker.clone(),
            )
            .await?;
        }
        None => {
            info!("No set found for sticker {sticker_unique_id}");
        }
    }
    Ok(())
}

async fn refetch_all_sets(
    database: Database,
    bot: Bot,
    worker: WorkerPool,
    admin_id: UserId,
) -> Result<(), BotError> {
    let start = chrono::Utc::now();
    let set_names = database.get_all_set_ids().await?;
    let total = set_names.len();
    let updated_to_send: usize = 1.max(200.min(total / 10));
    let message = bot
        .send_markdown(
            admin_id,
            Markdown::escaped(format!("Preparing to fetch {total} sticker sets")),
        )
        .await?;
    let message_id = message.id;
    for (i, set_name) in set_names.into_iter().enumerate() {
        import_all_stickers_from_set(
            set_name,
            None,
            false,
            bot.clone(),
            database.clone(),
            worker.clone(),
        )
        .await?;
        if i % (total / updated_to_send) == 0 {
            let elapsed = chrono::Utc::now() - start;
            bot.edit_message_markdown(
                admin_id,
                message_id,
                Markdown::escaped(format!(
                    "Fetched {} of {total} sticker sets in {} seconds (= {} minutes)",
                    i + 1,
                    elapsed.num_seconds(),
                    elapsed.num_minutes()
                )),
            )
            .await?;
        }
    }
    let elapsed = chrono::Utc::now() - start;
    bot.edit_message_markdown(
        admin_id,
        message_id,
        Markdown::escaped(format!(
            "Fetched {total} sticker sets in {} seconds (= {} minutes)",
            elapsed.num_seconds(),
            elapsed.num_minutes()
        )),
    )
    .await?;
    Ok(())
}

async fn refetch_scheduled(
    count: i64,
    database: Database,
    bot: Bot,
    worker: WorkerPool,
) -> Result<(), BotError> {
    let set_names = database.get_n_least_recently_fetched_set_ids(count).await?;
    for (i, set_name) in set_names.into_iter().enumerate() {
        import_all_stickers_from_set(
            set_name,
            None,
            false,
            bot.clone(),
            database.clone(),
            worker.clone(),
        )
        .await?;
    }
    Ok(())
}

async fn send_report(database: Database, bot: Bot, admin_id: UserId) -> Result<(), BotError> {
    let counts = database.get_stats().await?;
    let stats = database.get_admin_stats().await?;
    let taggings = database.get_user_tagging_stats_24_hours().await?;
    let age = stats
        .least_recently_fetched_set_age
        .map_or("fetch error".to_string(), |age| {
            format!("{}", age.num_hours())
        });
    let mut text = format!(
        "Daily Report:
- {} stickers ({} sets) with {} taggings
- {} sets fetched within 24 hours
- least recently fetched set age: {} hours

user taggings (24 hours):
",
        counts.stickers,
        counts.sets,
        counts.taggings,
        stats.number_of_sets_fetched_in_24_hours,
        age
    );
    for (user, stats) in taggings {
        match user {
            None => writeln!(
                text,
                "- no user: {} added, {} removed",
                stats.added_tags, stats.removed_tags
            )?,
            Some(user_id) => writeln!(
                text,
                "- user {}: {} added, {} removed",
                user_id, stats.added_tags, stats.removed_tags
            )?,
        };
        // TODO: add inline keyboard button for every user (until it reaches the telegram button limit)
    }

    let result = bot.send_markdown(admin_id, Markdown::escaped(text)).await?;
    Ok(())
}

impl Command {
    pub(super) async fn execute(
        self,
        bot: Bot,
        admin_id: UserId,
        worker: WorkerPool,
        database: Database,
    ) -> Result<(), BotError> {
        match self {
            Self::SendMessageToAdmin { msg, source_user } => {
                send_message_to_admin(msg, source_user, bot, admin_id).await
            }
            Self::ProcessStickerSet {
                set_name,
                source_user,
            } => {
                // TODO: retry on error
                // TODO: add parameter ignore_last_fetched
                import_all_stickers_from_set(
                    set_name,
                    source_user,
                    false,
                    bot.clone(),
                    database.clone(),
                    worker.clone(),
                )
                .await
            }
            Self::ProcessSetOfSticker {
                sticker_unique_id,
                source_user,
            } => {
                process_set_of_sticker(sticker_unique_id, source_user, database, bot, worker).await
            }
            Self::RefetchAllSets => refetch_all_sets(database, bot, worker, admin_id).await,
            Self::RefetchScheduled { count } => {
                refetch_scheduled(count, database, bot, worker).await
            }
            Self::SendReport => send_report(database, bot, admin_id).await,
            Self::SendExport => send_database_export_to_chat(admin_id.into(), database, bot).await,
        }
    }
}