use std::sync::Arc;
use std::time::Duration;

use teloxide::types::UserId;
use tokio::time::sleep;
use tracing::Instrument;

use crate::bot::Bot;
use crate::bot::report_periodic_task_error;

use crate::Config;
use crate::bot::InternalError;
use crate::database::Database;
use crate::inference::text_to_clip_embedding;
use crate::message::send_database_export_to_chat;
use crate::qdrant::VectorDatabase;
use crate::services::ExternalTelegramService;
use crate::services::ImportService;
use crate::services::Services;
use crate::simple_bot_api;

use super::TagManagerService;
use super::send_daily_report;

pub fn start_periodic_tasks(
    bot: Bot,
    database: Database,
    config: Arc<Config>,
    tag_manager: TagManagerService,
    vector_db: VectorDatabase,
    services: Services,
) {
    let bot_clone = bot.clone();
    let database_clone = database;
    let admin_id = config.get_admin_user_id();
    let config_clone = config;
    let vector_db_clone = vector_db.clone();
    let services_clone = services.clone();

    // TODO: make intervals and counts configurable

    let bot = bot_clone.clone();
    let database = database_clone.clone();
    let paths = config_clone.clone();
    let vector_db = vector_db_clone.clone();
    let services = services_clone.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_mins(15)).await;
            let span = tracing::info_span!("periodic_refetch_stickers");
            let bot = bot.clone();
            let database = database.clone();
            let paths = paths.clone();
            let vector_db = vector_db.clone();
            let services = services.clone();
            async move {
                // TODO: make this configurable
                let result = refetch_stickers(
                    paths.periodic_refetch_batch_size,
                    database.clone(),
                    bot.clone(),
                    paths.clone(),
                    vector_db.clone(),
                    services.import.clone(),
                )
                .await;
                report_periodic_task_error(result);
            }
            .instrument(span)
            .await;
            // span.exit();
        }
    });

    let services = services_clone.clone();
    let database = database_clone.clone();
    tokio::spawn(async move {
        let mut offset = 0;
        loop {
            let span = tracing::info_span!("periodic_discover_stickers");
            let services = services.clone();
            let database = database.clone();
            let (new_offset, result_len, should_wait_longer) = async move {
                let result = discover_stickers(offset, services.telegram, database, services.import).await;
                match result {
                    Ok((new_offset, result_len)) => (new_offset, result_len, false),
                    Err(err) => {
                        tracing::error!("periodic task error: {err:?}");
                        (offset, 0, true)
                    }
                }
            }
            .instrument(span)
            .await;
            if result_len == 0 && new_offset != offset && !should_wait_longer {
                sleep(Duration::from_secs(5)).await;
            } else {
                sleep(Duration::from_mins(5)).await;
                // TODO: i should really add a config for the automatic tasks
            }
            offset = new_offset;
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
                let result =
                    send_daily_report(database.clone(), bot.clone(), admin_id)
                        .await;
                report_periodic_task_error(result);
            }
            .instrument(span)
            .await;
            sleep(Duration::from_hours(24)).await;
        }
    });

    let bot = bot_clone;
    let database = database_clone.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_hours(7 * 24)).await;
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
            sleep(Duration::from_hours(23)).await;
        }
    });

    let vector_db = vector_db_clone.clone();
    let tag_manager_clone = tag_manager.clone();
    let config = config_clone.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_hours(24 * 2)).await;
            let span = tracing::info_span!("periodic_tag_insertion");
            // TODO: do daily; also refetch e6 tags
            let vector_db = vector_db.clone();
            let tag_manager_clone = tag_manager_clone.clone();
            let config = config.clone();
            async move {
                let result =
                    insert_tags(vector_db.clone(), tag_manager_clone.clone(), config.clone()).await;
                report_periodic_task_error(result);
            }
            .instrument(span)
            .await;
        }
    });

    let database = database_clone.clone();
    let vector_db = vector_db_clone.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(10)).await;
            let span = tracing::info_span!("periodic_sticker_file_cleanup");
            let database = database.clone();
            let vector_db = vector_db.clone();
            async move {
                let result = clean_up_sticker_files(database.clone(), vector_db.clone()).await;
                report_periodic_task_error(result);
            }
            .instrument(span)
            .await;
            sleep(Duration::from_hours(23)).await;
        }
    });
}

#[tracing::instrument(skip(tg_service, importer, database))]
async fn discover_stickers(
    offset: usize,
    tg_service: ExternalTelegramService,
    database: Database,
    importer: ImportService,
) -> Result<(usize, usize), InternalError> {
    let limit = 20;
    if importer.is_busy() {
        tracing::info!("importer is busy, skipping import");
        return Ok((offset, 0));
    }
    let Some(result) = tg_service
        .list_discovered_sticker_packs(offset, limit)
        .await
    else {
        return Ok((offset, 0));
    };
    let mut new_packs = 0;
    for pack in &result.packs {
        let existing = database.get_sticker_set_by_id(&pack.short_name).await?;
        if existing.is_none() {
            let banned = database.is_sticker_set_banned(&pack.short_name).await?;
            if !banned {
                importer
                    .queue_sticker_set_import(&pack.short_name, false, None, pack.telegram_pack_id)
                    .await;
                new_packs += 1;
            }
        }
    }

    if result.packs.len() == limit {
        Ok((offset + limit, new_packs))
    } else {
        Ok((offset, new_packs))
    }
}

#[tracing::instrument(skip(database))]
async fn clean_up_sticker_files(
    database: Database,
    vector_db: VectorDatabase,
) -> Result<(), InternalError> {
    let deleted_file_ids = database
        .clean_up_sticker_files_without_stickers_and_without_tags()
        .await?;
    if !deleted_file_ids.is_empty() {
        vector_db.delete_stickers(deleted_file_ids).await?;
    }
    Ok(())
}

#[tracing::instrument(skip(vector_db, tag_manager, config))]
async fn insert_tags(
    vector_db: VectorDatabase,
    tag_manager: TagManagerService,
    config: Arc<Config>,
) -> Result<(), InternalError> {
    tag_manager.recompute().await;

    let tags = tag_manager.get_tags();
    let aliases = tag_manager.get_aliases();

    for tag_or_alias in tags.into_iter().chain(aliases) {
        let embedding =
            text_to_clip_embedding(tag_or_alias.to_string(), config.inference_url.clone()).await?;
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
    tag_manager: TagManagerService,
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
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    Ok(())
}

#[tracing::instrument(skip(database, bot, config, vector_db, importer))]
async fn refetch_stickers(
    count: u64,
    database: Database,
    bot: Bot,
    config: Arc<Config>,
    vector_db: VectorDatabase,
    importer: ImportService,
) -> Result<(), InternalError> {
    if config.is_readonly {
        return Ok(());
    }
    let set_names = database
        .get_n_least_recently_fetched_set_ids(count as i64)
        .await?;
    if importer.is_busy() {
        return Ok(());
    }
    for (i, set_name) in set_names.into_iter().enumerate() {
        importer
            .queue_sticker_set_import(&set_name, false, None, None)
            .await;
    }
    let stats = database.get_stats().await?;
    let percentage_tagged = stats.tagged_stickers as f32 / stats.stickers as f32 * 100.0;
    simple_bot_api::set_my_short_description(
        &config.telegram_bot_token, &format!("I organize {} furry sticker sets 💚 {} taggings 💚 {} stickers ({:.1}% tagged) 💚  uwu", stats.sets, stats.taggings, stats.stickers, percentage_tagged)).await?;
    simple_bot_api::set_my_description(
        &config.telegram_bot_token, &format!("Hi, I am {} and I organize furry sticker sets!

To use me, type @{} followed some tags in any chat to find one of the {} already tagged stickers. 

If you know some stickers I might not be aware of, want to help tag the remaining {} stickers, or want to find stickers related to ones you already have, chat with me 💚", &config.bot_display_name, &config.telegram_bot_username.to_lowercase(), stats.tagged_stickers, stats.stickers - stats.tagged_stickers)).await?;
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    Ok(())
}
