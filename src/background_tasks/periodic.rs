
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

pub fn start_periodic_tasks(bot: Bot, admin_id: UserId, database: Database, paths: Arc<Paths>, worker: AnalysisWorker) {
    let bot_clone = bot.clone();
    let database_clone = database.clone();
    let paths_clone = paths.clone();
    let error_handler_clone = BackgroundTaskErrorHandler {
        bot,
        admin_id
    };

    // TODO: make intervals and counts configurable

    let bot = bot_clone.clone();
    let database = database_clone.clone();
    let paths = paths_clone.clone();
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

    let bot = bot_clone.clone();
    let database = database_clone.clone();
    let error_handler = error_handler_clone.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::days(7).to_std().expect("no overflow")).await;
            let result = send_database_export_to_chat(admin_id.into(), database.clone(), bot.clone()).await;
            error_handler.handle(result).await;
        }
    });
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
        import_all_stickers_from_set(
            set_name,
            false,
            bot.clone(),
            database.clone(),
        )
        .await?;
    }
    analyze_n_stickers(database.clone(), bot.clone(), 100 * count, paths.clone(), worker).await?;
    Ok(())
}
