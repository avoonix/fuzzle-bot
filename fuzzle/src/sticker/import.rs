use std::{future::IntoFuture, sync::Arc};

use itertools::Itertools;
use regex::Regex;
use teloxide::{requests::Requester, types::UserId};
use tokio::task;
use tracing::{info, warn, Instrument};

use crate::{
    background_tasks::BackgroundTaskExt,
    bot::{Bot, BotError, InternalError, RequestContext, UserError},
    database::Database,
    qdrant::VectorDatabase,
    util::{decode_sticker_set_id, is_wrong_file_id_error, Emoji},
    Config,
};

use super::{
    analyze_sticker, automerge,
    download::{fetch_sticker_file, FileKind},
    hash::calculate_sticker_file_hash,
};

#[tracing::instrument(skip(database, bot, config, vector_db), err(Debug))]
pub async fn import_all_stickers_from_set(
    set_id: &str,
    ignore_last_fetched: bool,
    bot: Bot,
    database: Database,
    config: Arc<Config>,
    vector_db: VectorDatabase,
    user_id: Option<UserId>,
) -> Result<(), InternalError> {
    if !ignore_last_fetched {
        let fetched_recently = database
            .get_sticker_set_by_id(&set_id)
            .await?
            .and_then(|set| set.last_fetched)
            .map_or(false, |last_fetched| {
                let now = chrono::Utc::now().naive_utc();
                let time_since_last_fetch = now - last_fetched;
                time_since_last_fetch < chrono::Duration::hours(2)
            });
        if fetched_recently {
            return Ok(());
        }
    }

    let set = bot
        .get_sticker_set(set_id)
        .into_future()
        .instrument(tracing::info_span!("telegram_bot_get_sticker_set"))
        .await;

    let set = match set {
        Ok(set) => set,
        // does not exist error
        Err(teloxide::RequestError::Api(teloxide::ApiError::InvalidStickersSet)) => {
            // sticker set has been deleted
            database.delete_sticker_set(&set_id).await?; // TODO: due to case sensitivity, this might not actually delete the set
            return Ok(());
        }
        Err(e) => {
            return Err(e.into());
        }
    };

    if !set.is_regular() {
        database.delete_sticker_set(&set.name).await?;
        return Ok(()); // ignore custom emojis as the bot is unable to send those
    }

    fetch_sticker_set_and_save_to_db(set, bot, database, config, vector_db, user_id).await?;

    Ok(())
}

// steps:
// 1. user sends sticker pack link -> just queue the whole set in the worker and message the user when its done
// 2. user sends individual sticker ->
//        1. check if sticker is in database; if it is, queue the pack fetch in the worker; done
//        2. if the sticker is not in the database, continue to step 3;
//        3. try to insert the sticker set in the database (if it did not exist before, last_fetch should be null)
//        4. download and hash the sticker file, and insert it into the database
//        5. queue the whole set for crawling
//        6. (bonus) if an error happens, still crawl the rest of the pack (some stickers don't load), and inform an admin about the error

#[tracing::instrument(skip(request_context, sticker), fields(sticker_unique_id = sticker.file.unique_id))]
pub async fn import_individual_sticker_and_queue_set(
    sticker: teloxide::types::Sticker,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let set_id = sticker.set_name.clone().ok_or_else(|| UserError::StickerNotPartOfSet)?;

    if !sticker.is_regular() {
        return Err(UserError::UnsupportedStickerType.into());
    }

    let sticker_in_database = request_context
        .database
        .get_sticker_by_id(&sticker.file.unique_id)
        .await?
        .is_some();
    if sticker_in_database {
        if !request_context.importer.is_busy() {
            request_context.importer.queue_sticker_set_import(&set_id, false, Some(request_context.user_id())).await;
        }
        return Ok(());
    }

    // sticker is not in database; ensure that the set exists before inserting the sticker
    request_context
        .database
        .upsert_sticker_set(&set_id, request_context.user.id)
        .await?;
    info!("sticker not in database");

    if request_context.config.is_readonly { return Ok(()); }

    fetch_sticker_and_save_to_db(
        sticker.clone(),
        set_id.clone(),
        request_context.bot.clone(),
        request_context.database.clone(),
    )
    .await?;

    request_context.importer.queue_sticker_set_import(&set_id, true, Some(request_context.user_id())).await;

    request_context
        .analyze_sticker(sticker.file.unique_id.clone())
        .await;

    Ok(())
}

#[tracing::instrument(skip(database, bot, sticker))]
async fn fetch_sticker_and_save_to_db(
    sticker: teloxide::types::Sticker,
    set_name: String,
    bot: Bot,
    database: Database,
) -> Result<(), InternalError> {
    let emoji = sticker.emoji.map(|e| Emoji::new_from_string_single(&e));

    let (buf, file) = fetch_sticker_file(sticker.file.id.clone(), bot.clone()).await?;
    let thread_span =
        tracing::info_span!("spawn_blocking_calculate_sticker_file_hash").or_current();
    let file_hash = task::spawn_blocking(move || {
        let _entered = thread_span.entered();
        calculate_sticker_file_hash(buf)
    })
    .await??;

    let canonical_file_hash = database.find_canonical_sticker_file_id(&file_hash).await?;

    database
        .create_file(
            &canonical_file_hash,
            sticker.thumb.map(|thumb| thumb.file.id),
            match sticker.format {
                teloxide::types::StickerFormat::Raster => crate::database::StickerType::Static,
                teloxide::types::StickerFormat::Animated => crate::database::StickerType::Animated,
                teloxide::types::StickerFormat::Video => crate::database::StickerType::Video,
            },
        )
        .await?;
    database
        .create_sticker(
            &sticker.file.unique_id,
            &sticker.file.id,
            emoji,
            &set_name,
            &canonical_file_hash,
        )
        .await?;
    Ok(())
}

#[tracing::instrument(skip(database, bot, set, config, vector_db), fields(set.name = set.name))]
async fn fetch_sticker_set_and_save_to_db(
    set: teloxide::types::StickerSet,
    bot: Bot,
    database: Database,
    config: Arc<Config>,
    vector_db: VectorDatabase,
    user_id: Option<UserId>,
) -> Result<(), InternalError> {
    // TODO: result should be how many stickers were added/removed/updated

    let creator_id = decode_sticker_set_id(set.__custom__id.clone())?.owner_id;

    database
        .upsert_sticker_set_with_title_and_creator(&set.name, &set.title, creator_id, user_id.map(|id| id.0 as i64))
        .await?;
    let saved_stickers = database.get_all_stickers_in_set(&set.name).await?;

    let re = Regex::new(r"(\W|^)@([_a-zA-Z0-9]+)\b").expect("static regex to compile");
    for (_, [_, username]) in re.captures_iter(&set.title).map(|c| c.extract()) {
        database.add_username(username).await?;
    }

    let all_saved_sticker_file_hashes = saved_stickers
        .iter()
        .map(|s| s.sticker_file_id.to_string())
        .collect_vec();
    let missing_file_hashes = vector_db
        .find_missing_stickers(all_saved_sticker_file_hashes)
        .await?;

    let missing_sticker_ids = database
        .get_some_sticker_ids_for_sticker_file_ids(missing_file_hashes)
        .await?;
    for missing_sticker_id in missing_sticker_ids {
        analyze_sticker(
            missing_sticker_id.sticker_id,
            database.clone(),
            bot.clone(),
            config.clone(),
            vector_db.clone(),
        )
        .await?;
    }

    //  let analysis = database.get_n_stickers_with_missing_analysis(n).await?;
    //     let mut changed = false;
    //     for analysis in analysis {
    //         if analyze_sticker(analysis, database.clone(), bot.clone(), paths.clone(), vector_db.clone()).await? {
    //             changed = true;
    //         }
    //     }

    let stickers_not_in_database_yet = set.stickers.clone().into_iter().filter(|sticker| {
        saved_stickers
            .iter()
            .all(|s| s.id != sticker.file.unique_id)
    });
    // TODO: find out which stickers are missing embeddings; find out which stickers need to be updated (is_animated)?

    if config.is_readonly { return Ok(()); }

    // todo: tag animated?
    for sticker in stickers_not_in_database_yet {
        let result = fetch_sticker_and_save_to_db(
            sticker.clone(),
            set.name.clone(),
            bot.clone(),
            database.clone(),
        )
        .await;

        match result {
            Err(InternalError::Teloxide(teloxide::RequestError::Api(api_err)))
                if is_wrong_file_id_error(&api_err) =>
            {
                warn!("invalid file_id for a sticker, continuing");
            }
            Err(other) => {
                if other.is_timeout_error() {
                    warn!("sticker fetch timed out, continuing");
                } else {
                    return Err(other);
                }
            }
            Ok(()) => {}
        }

        analyze_sticker(
            sticker.file.unique_id.clone(),
            database.clone(),
            bot.clone(),
            config.clone(),
            vector_db.clone(),
        )
        .await?;
    }
    for sticker in saved_stickers.clone() {
        let Some(s) = set.stickers.iter().find(|s| s.file.unique_id == sticker.id) else {
            continue;
        };
        if s.file.id != sticker.telegram_file_identifier {
            // TODO: might be updated too frequently
            database
                .update_sticker(sticker.id, s.file.id.clone())
                .await?;
        }
    }

    let deleted_stickers = saved_stickers
        .iter()
        .filter(|s| !set.stickers.iter().any(|s2| s2.file.unique_id == s.id))
        .collect_vec();

    for sticker in deleted_stickers {
        database.delete_sticker(&sticker.id).await?; // only deletes the sticker, not the file
                                                     // TODO: periodically clean up no longer referenced files
    }

    for sticker in set.stickers.iter().filter(|s| s.is_raster()) {
        let result = automerge(
            &sticker.file.unique_id,
            database.clone(),
            vector_db.clone(),
            bot.clone(),
        )
        .await; // TODO: this might happen too frequently
        match result {
            Err(InternalError::UnexpectedNone { type_name }) => {
                tracing::warn!("a {type_name} is unexpectedly none");
            }
            other => other?,
        }
    }

    database.update_last_fetched(set.name.clone()).await?;

    Ok(())
}
