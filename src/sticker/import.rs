use itertools::Itertools;
use log::{info, warn};
use teloxide::{requests::Requester, types::UserId};
use tokio::task;

use crate::{
    bot::{Bot, BotError},
    database::Database,
    util::{is_wrong_file_id_error, Emoji},
    worker::WorkerPool,
};

use super::{
    download::{fetch_sticker_file, FileKind},
    hash::calculate_sticker_file_hash,
};

// TODO: test sticker/sticker set deletion

pub async fn import_all_stickers_from_set(
    set_name: String,
    user_id: Option<UserId>,
    ignore_last_fetched: bool,
    bot: Bot,
    database: Database,
    worker: WorkerPool,
) -> Result<(), BotError> {
    if !ignore_last_fetched {
        let fetched_recently = database
            .sticker_set_fetched_within_duration(set_name.clone(), chrono::Duration::hours(24))
            .await?;
        if fetched_recently {
            info!("set fetched recently");
            return Ok(());
        }
    }

    let set = match bot.get_sticker_set(set_name.clone()).await {
        Ok(set) => set,
        // does not exist error
        Err(teloxide::RequestError::Api(teloxide::ApiError::InvalidStickersSet)) => {
            // sticker set has been deleted
            database.delete_sticker_set(set_name).await?; // TODO: due to case sensitivity, this might not actually delete the set
            return Ok(());
        }
        Err(e) => {
            return Err(e.into());
        }
    };
    let set_name = set.name.clone(); // passed set name might be wrong casing (set names in databases are case sensitive)

    if let Some(user_id) = user_id {
        notify_admin_if_set_new(database.clone(), worker, set_name, user_id).await?;
    }

    fetch_sticker_set_and_save_to_db(set, bot, database).await?;

    Ok(())
}

async fn notify_admin_if_set_new(
    database: Database,
    worker: WorkerPool,
    set_name: String,
    user_id: UserId,
) -> anyhow::Result<()> {
    let saved_set = database.get_sticker_set(set_name.clone()).await?;
    if saved_set.is_none() {
        // TODO: this is sent even if the set is banned -> check if banned
        worker
            .dispatch_message_to_admin(
                user_id,
                crate::worker::AdminMessage::StickerSetAdded {
                    set_name: set_name.clone(),
                },
            )
            .await;
    }
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

pub async fn import_individual_sticker_and_queue_set(
    sticker: teloxide::types::Sticker,
    user_id: UserId,
    bot: Bot,
    database: Database,
    worker: WorkerPool,
) -> Result<(), BotError> {
    let Some(set_name) = sticker.set_name.clone() else {
        return Err(anyhow::anyhow!(
            "sticker {} does not have a set name",
            &sticker.file.unique_id
        ))?;
    };
    let sticker_in_database = database
        .get_sticker(sticker.file.unique_id.clone())
        .await?
        .is_some();
    if sticker_in_database {
        info!("sticker in database, queuing set");
        worker.process_sticker_set(Some(user_id), set_name).await;
        return Ok(());
    }

    // sticker is not in database; ensure that the set exists before inserting the sticker
    notify_admin_if_set_new(database.clone(), worker.clone(), set_name.clone(), user_id).await?;
    database.create_sticker_set(set_name.clone(), None).await?;
    info!("sticker not in database");

    fetch_sticker_and_save_to_db(sticker, set_name.clone(), bot, database).await?;

    worker.process_sticker_set(Some(user_id), set_name).await; // TODO: ignore last fetched? (in case multiple new stickers were added within the last 24 hours?)

    Ok(())
}

async fn fetch_sticker_and_save_to_db(
    sticker: teloxide::types::Sticker,
    set_name: String,
    bot: Bot,
    database: Database,
) -> Result<(), BotError> {
    info!("fetching sticker from set {set_name}");
    let emojis = sticker.emoji.map(|e| Emoji::parse(&e)).unwrap_or_default();

    let (buf, file) = fetch_sticker_file(sticker.file.id.clone(), bot.clone()).await?;
    let file_kind = FileKind::from(file.path.as_str());

    let file_hash =
        task::spawn_blocking(move || calculate_sticker_file_hash(buf, file_kind)).await??;

    database.create_file(file_hash.clone()).await?;
    database
        .create_sticker(
            sticker.file.unique_id,
            sticker.file.id,
            emojis,
            set_name,
            file_hash.clone(),
        )
        .await?;
    if let Some(thumb) = sticker.thumb {
        database.update_thumbnail(file_hash, thumb.file.id).await?;
    }
    Ok(())
}

async fn fetch_sticker_set_and_save_to_db(
    set: teloxide::types::StickerSet,
    bot: Bot,
    database: Database,
) -> Result<(), BotError> {
    // TODO: result should be how many stickers were added/removed/updated
    info!("fetching set {}", &set.name);

    database
        .create_sticker_set(set.name.clone(), Some(set.title.clone()))
        .await?; // does not update last_fetched
    let saved_stickers = database.get_all_stickers_in_set(set.name.clone()).await?;
    let stickers_not_in_database_yet = set.stickers.clone().into_iter().filter(|sticker| {
        saved_stickers
            .iter()
            .all(|s| s.id != sticker.file.unique_id)
    });
    // TODO: find out which stickers are missing visual hashes?

    // todo: tag animated?
    for sticker in stickers_not_in_database_yet {
        let result =
            fetch_sticker_and_save_to_db(sticker, set.name.clone(), bot.clone(), database.clone())
                .await;

        match result {
            Err(BotError::Timeout) => {
                warn!("sticker fetch timed out, continuing");
            }
            Err(BotError::Teloxide(teloxide::RequestError::Api(api_err)))
                if is_wrong_file_id_error(&api_err) =>
            {
                warn!("invalid file_id for a sticker, continuing");
            }
            Err(other) => return Err(other),
            Ok(()) => {}
        }
    }
    for sticker in saved_stickers.clone() {
        let Some(s) = set.stickers.iter().find(|s| s.file.unique_id == sticker.id) else {
            continue;
        };
        if s.file.id != sticker.file_id {
            database
                .update_sticker(sticker.id, s.file.id.clone())
                .await?;
        }
        if let Some(thumb) = s.thumb.clone() {
            // TODO: check if thumbnail is already up to date
            database
                .update_thumbnail(sticker.file_hash, thumb.file.id)
                .await?;
        }
    }

    let deleted_stickers = saved_stickers
        .iter()
        .filter(|s| !set.stickers.iter().any(|s2| s2.file.unique_id == s.id))
        .collect_vec();

    for sticker in deleted_stickers {
        database.delete_sticker(sticker.id.clone()).await?; // only deletes the sticker, not the file
                                                            // TODO: periodically clean up no longer referenced files
    }

    database.update_last_fetched(set.name.clone()).await?;

    Ok(())
}
