use std::sync::Arc;

use chrono::Duration;

use teloxide::types::UserId;
use tokio::time::sleep;
use tracing::Instrument;

use crate::bot::report_periodic_task_error;
use crate::bot::Bot;

use crate::bot::InternalError;
use crate::database::Database;
use crate::inference::text_to_clip_embedding;
use crate::message::send_database_export_to_chat;
use crate::qdrant::VectorDatabase;
use crate::sticker::import_all_stickers_from_set;
use crate::tags::TagManager;
use crate::Config;

use super::send_daily_report;

pub fn start_periodic_tasks(
    bot: Bot,
    database: Database,
    config: Arc<Config>,
    tag_manager: Arc<TagManager>,
    vector_db: VectorDatabase,
) {
    let bot_clone = bot.clone();
    let database_clone = database;
    let admin_id = config.get_admin_user_id();
    let config_clone = config;
    let vector_db_clone = vector_db.clone();

    // TODO: make intervals and counts configurable

    let bot = bot_clone.clone();
    let database = database_clone.clone();
    let paths = config_clone.clone();
    let vector_db = vector_db_clone.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::minutes(4).to_std().expect("no overflow")).await;
            let span = tracing::info_span!("periodic_refetch_stickers");
            let bot = bot.clone();
            let database = database.clone();
            let paths = paths.clone();
            let vector_db = vector_db.clone();
            async move {
                // fetching 69 sets every 10 minutes is about 10000 sets per day
                let result = refetch_stickers(
                    69,
                    database.clone(),
                    bot.clone(),
                    paths.clone(),
                    vector_db.clone(),
                )
                .await;
                report_periodic_task_error(result);
            }
            .instrument(span)
            .await;
            // span.exit();
        }
    });

    let bot = bot_clone.clone();
    let database = database_clone.clone();
    tokio::spawn(async move {
        loop {
            let span = tracing::info_span!("periodic_daily_report");
            let bot = bot.clone();
            let database = database.clone();
            async move {
                let result = send_daily_report(database.clone(), bot.clone(), admin_id).await;
                report_periodic_task_error(result);
            }
            .instrument(span)
            .await;
            sleep(Duration::hours(24).to_std().expect("no overflow")).await;
        }
    });

    let bot = bot_clone;
    let database = database_clone.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::days(7).to_std().expect("no overflow")).await;
            let span = tracing::info_span!("periodic_database_export");
            let bot = bot.clone();
            let database = database.clone();
            async move {
                let result =
                    send_database_export_to_chat(admin_id.into(), database.clone(), bot.clone())
                        .await;
                report_periodic_task_error(result);
            }
            .instrument(span)
            .await;
        }
    });

    let database = database_clone.clone();
    let tag_manager_clone = tag_manager.clone();
    tokio::spawn(async move {
        // TODO: download new tags from e621 periodically
        loop {
            let span = tracing::info_span!("periodic_tag_implication_fix");
            let database = database.clone();
            let tag_manager_clone = tag_manager_clone.clone();
            async move {
                let result =
                    fix_missing_tag_implications(database.clone(), tag_manager_clone.clone()).await;
                report_periodic_task_error(result);
            }
            .instrument(span)
            .await;
            sleep(Duration::hours(23).to_std().expect("no overflow")).await;
        }
    });

    let vector_db = vector_db_clone.clone();
    let tag_manager_clone = tag_manager.clone();
    let config = config_clone.clone();
    tokio::spawn(async move {
        loop {
            let span = tracing::info_span!("periodic_tag_insertion");
            // TODO: do daily; also refetch e6 tags
            let vector_db = vector_db.clone();
            let tag_manager_clone = tag_manager_clone.clone();
            let config = config.clone();
            async move {
                let result = insert_tags(vector_db.clone(), tag_manager_clone.clone(), config.clone()).await;
                report_periodic_task_error(result);
            }
            .instrument(span)
            .await;
            sleep(Duration::hours(1).to_std().expect("no overflow")).await;
        }
    });
}

#[tracing::instrument(skip(vector_db, tag_manager, config))]
async fn insert_tags(
    vector_db: VectorDatabase,
    tag_manager: Arc<TagManager>,
    config: Arc<Config>,
) -> Result<(), InternalError> {
    for tag_or_alias in tag_manager.iter_all() {
        let embedding = text_to_clip_embedding(tag_or_alias.to_string(), config.inference_url.clone()).await?;
        vector_db
            .insert_tag(embedding, tag_or_alias.to_string())
            .await?;
        // TODO: insert both tag and alias
    }

    Ok(())
}

#[tracing::instrument(skip(database, tag_manager))]
async fn fix_missing_tag_implications(
    database: Database,
    tag_manager: Arc<TagManager>,
) -> Result<(), InternalError> {
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
                    vec![],
                    1000,
                    0,
                    crate::database::Order::LatestFirst,
                )
                .await?;
            if !result.is_empty() {
                for sticker in result {
                    let Some(file) = database.get_sticker_file_by_sticker_id(&sticker.id).await?
                    else {
                        continue;
                    };
                    database
                        .tag_file(&file.id, &vec![implication.clone()], None)
                        .await?;
                    sleep(Duration::seconds(1).to_std().expect("no overflow")).await;
                }
            }
        }
    }

    Ok(())
}

#[tracing::instrument(skip(database, bot, config, vector_db))]
async fn refetch_stickers(
    count: i64,
    database: Database,
    bot: Bot,
    config: Arc<Config>,
    vector_db: VectorDatabase,
) -> Result<(), InternalError> {
    let set_names = database.get_n_least_recently_fetched_set_ids(count).await?;
    for (i, set_name) in set_names.into_iter().enumerate() {
        import_all_stickers_from_set(
            &set_name,
            true,
            bot.clone(),
            database.clone(),
            config.clone(),
            vector_db.clone(),
        )
        .await?;
    }
    Ok(())
}
