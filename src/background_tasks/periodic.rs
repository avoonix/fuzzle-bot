use std::sync::Arc;

use chrono::Duration;

use teloxide::types::UserId;
use tokio::time::sleep;

use crate::bot::log_error_and_send_to_admin;
use crate::bot::Bot;

use crate::bot::BotError;
use crate::database::Database;
use crate::message::send_database_export_to_chat;
use crate::sticker::analyze_n_stickers;
use crate::sticker::import_all_stickers_from_set;
use crate::tags::TagManager;
use crate::Paths;

use super::send_daily_report;
use super::AnalysisWorker;

#[derive(Clone)]
struct BackgroundTaskErrorHandler {
    bot: Bot,
    admin_id: UserId,
}

impl BackgroundTaskErrorHandler {
    async fn handle<T>(&self, result: Result<T, BotError>) {
        match result {
            Ok(_) => {}
            Err(err) => log_error_and_send_to_admin(err, self.bot.clone(), self.admin_id).await,
        }
    }
}

pub fn start_periodic_tasks(
    bot: Bot,
    admin_id: UserId,
    database: Database,
    paths: Arc<Paths>,
    worker: AnalysisWorker,
    tag_manager: Arc<TagManager>,
) {
    let bot_clone = bot.clone();
    let database_clone = database;
    let paths_clone = paths;
    let error_handler_clone = BackgroundTaskErrorHandler { bot, admin_id };

    // TODO: make intervals and counts configurable

    let bot = bot_clone.clone();
    let database = database_clone.clone();
    let paths = paths_clone;
    let error_handler = error_handler_clone.clone();
    tokio::spawn(async move {
        loop {
            // fetching 69 sets every 10 minutes is about 10000 sets per day
            sleep(Duration::minutes(4).to_std().expect("no overflow")).await;
            let result = refetch_and_analyze_stickers(
                69,
                database.clone(),
                bot.clone(),
                paths.clone(),
                worker.clone(),
            )
            .await;
            error_handler.handle(result).await;
        }
    });

    let bot = bot_clone.clone();
    let database = database_clone.clone();
    let error_handler = error_handler_clone.clone();
    tokio::spawn(async move {
        loop {
            let result = send_daily_report(database.clone(), bot.clone(), admin_id).await;
            error_handler.handle(result).await;
            sleep(Duration::hours(24).to_std().expect("no overflow")).await;
        }
    });

    let bot = bot_clone;
    let database = database_clone.clone();
    let error_handler = error_handler_clone.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::days(7).to_std().expect("no overflow")).await;
            let result =
                send_database_export_to_chat(admin_id.into(), database.clone(), bot.clone()).await;
            error_handler.handle(result).await;
        }
    });

    let database = database_clone.clone();
    let error_handler = error_handler_clone.clone();
    tokio::spawn(async move {
        // TODO: download new tags from e621 periodically
        loop {
            let result = fix_missing_tag_implications(database.clone(), tag_manager.clone()).await;
            error_handler.handle(result).await;
            sleep(Duration::hours(23).to_std().expect("no overflow")).await;
        }
    });
}

async fn fix_missing_tag_implications(database: Database, tag_manager: Arc<TagManager>) -> Result<(), BotError> {
    let used_tags = database.get_used_tags().await?;
    for tag in used_tags {
        let Some(implications) = tag_manager.get_implications(&tag) else {
            continue;
        };
        for implication in implications {
            let result = database
                .get_stickers_for_tag_query(
                    vec![tag.clone()],
                    vec![implication.clone()],
                    1000,
                    0,
                    crate::database::Order::LatestFirst,
                )
                .await?;
            if !result.is_empty() {
                for sticker in result {
                    database
                        .tag_sticker(sticker.id, vec![implication.clone()], None)
                        .await?;
                    sleep(Duration::seconds(1).to_std().expect("no overflow")).await;
                }
            }
        }
    }

    Ok(())
}

async fn refetch_and_analyze_stickers(
    count: i64,
    database: Database,
    bot: Bot,
    paths: Arc<Paths>,
    worker: AnalysisWorker,
) -> Result<(), BotError> {
    let set_names = database.get_n_least_recently_fetched_set_ids(count).await?;
    for (i, set_name) in set_names.into_iter().enumerate() {
        import_all_stickers_from_set(set_name, false, bot.clone(), database.clone()).await?;
    }
    analyze_n_stickers(
        database.clone(),
        bot.clone(),
        100 * count,
        paths.clone(),
        worker,
    )
    .await?;
    Ok(())
}
