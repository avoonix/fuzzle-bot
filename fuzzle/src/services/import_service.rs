use std::sync::Arc;

use flume::Sender;
use itertools::Itertools;
use regex::Regex;
use teloxide::{prelude::Requester, types::UserId};
use tokio::task;
use tracing::Instrument;

use crate::{
    Config, bot::{Bot, BotError, InternalError, UserError, report_periodic_task_error}, database::{BanReason, Database, DatabaseError, StickerType}, inference::image_to_clip_embedding, qdrant::VectorDatabase, services::ExternalTelegramService, sticker::{automerge, calculate_color_histogram, calculate_sticker_file_hash, fetch_sticker_file}, util::{Emoji, Required, decode_sticker_set_id, is_wrong_file_id_error}
};

#[derive(Clone)]
pub struct ImportService {
    database: Database,
    config: Arc<Config>,
    bot: Bot,
    vector_db: VectorDatabase,
    tx: Sender<StickerSetFetchRequest>,
    tg_service: ExternalTelegramService,
}

struct StickerSetFetchRequest {
    set_id: String,
    ignore_last_fetched: bool,
    added_by_user_id: Option<UserId>,
    numeric_set_id: Option<i64>,
}

impl ImportService {
    #[tracing::instrument(skip(database, config, tg_service, bot, vector_db))]
    pub fn new(
        database: Database,
        config: Arc<Config>,
        bot: Bot,
        vector_db: VectorDatabase,
        tg_service: ExternalTelegramService,
    ) -> Self {
        metrics::describe_gauge!(
            "fuzzle_sticker_import_queue_length",
            metrics::Unit::Count,
            "Number of sticker sets in the import queue length"
        );
        let (tx, rx) = flume::unbounded();
        let service = Self {
            database,
            config,
            bot,
            vector_db,
            tx,
            tg_service,
        };
        {
            let service = service.clone();
            tokio::spawn(async move {
                while let Ok(received) = rx.recv_async().await {
                    metrics::gauge!("fuzzle_sticker_import_queue_length").set(rx.len() as f64);
                    let StickerSetFetchRequest {
                        set_id,
                        ignore_last_fetched,
                        added_by_user_id,
                        numeric_set_id,
                    } = received;
                    let result = service
                        .import_all_stickers_from_set(
                            &set_id,
                            ignore_last_fetched,
                            added_by_user_id,
                            numeric_set_id,
                        )
                        .await;
                    report_periodic_task_error(result);
                }
            });
        }
        service
    }

    pub async fn queue_sticker_set_import(
        &self,
        set_id: &str,
        ignore_last_fetched: bool,
        added_by_user_id: Option<UserId>,
        numeric_set_id: Option<i64>,
    ) {
        self.tx
            .send_async(StickerSetFetchRequest {
                set_id: set_id.to_string(),
                ignore_last_fetched,
                added_by_user_id,
                numeric_set_id,
            })
            .await
            .expect("channel must be open");
    }

    /// avoid adding already imported sticker sets to the queue if busy
    pub fn is_busy(&self) -> bool {
        self.tx.len() > 100
    }

    #[tracing::instrument(skip(self), err(Debug))]
    async fn import_all_stickers_from_set(
        &self,
        set_id: &str,
        ignore_last_fetched: bool,
        user_id: Option<UserId>,
        numeric_set_id: Option<i64>,
    ) -> Result<(), InternalError> {
        if !ignore_last_fetched {
            let fetched_recently = self
                .database
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

        let set = self
            .bot
            .get_sticker_set(set_id)
            .into_future()
            .instrument(tracing::info_span!("telegram_bot_get_sticker_set"))
            .await;

        let set = match set {
            Ok(set) => set,
            // does not exist error
            Err(teloxide::RequestError::Api(teloxide::ApiError::InvalidStickersSet)) => {
                // sticker set has been deleted
                self.database.delete_sticker_set(&set_id).await?; // TODO: due to case sensitivity, this might not actually delete the set
                return Ok(());
            }
            Err(e) => {
                return Err(e.into());
            }
        };

        if !set.is_regular() {
            self.database.delete_sticker_set(&set.name).await?;
            return Ok(()); // ignore custom emojis as the bot is unable to send those
        }

        self.fetch_sticker_set_and_save_to_db(set, user_id, numeric_set_id)
            .await?;

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

    #[tracing::instrument(skip(self, sticker), fields(sticker_unique_id = sticker.file.unique_id))]
    pub async fn import_individual_sticker_and_queue_set(
        &self,
        sticker: teloxide::types::Sticker,
        added_by_user_id: UserId,
    ) -> Result<(), BotError> {
        // TODO: should be internal error
        let set_id = sticker
            .set_name
            .clone()
            .ok_or_else(|| UserError::StickerNotPartOfSet)?;

        if !sticker.is_regular() {
            return Err(UserError::UnsupportedStickerType.into());
        }

        let sticker_in_database = self
            .database
            .get_sticker_by_id(&sticker.file.unique_id)
            .await?
            .is_some();
        if sticker_in_database {
            if !self.is_busy() {
                self.queue_sticker_set_import(
                    &set_id,
                    false,
                    Some(added_by_user_id),
                    None,
                )
                .await;
            }
            return Ok(());
        }

        // sticker is not in database; ensure that the set exists before inserting the sticker
        self
            .database
            .upsert_sticker_set(&set_id, Some(added_by_user_id.0 as i64))
            .await?;
        tracing::info!("sticker not in database");

        if self.config.is_readonly {
            return Ok(());
        }

        self.fetch_sticker_and_save_to_db( sticker.clone(), set_id.clone(),) .await?;

        self.queue_sticker_set_import(&set_id, true, Some(added_by_user_id), None)
            .await;

        self
            .analyze_sticker_background(sticker.file.unique_id.clone())
            .await;

        Ok(())
    }
    
    #[tracing::instrument(skip(self))]
    async fn analyze_sticker_background(&self, sticker_unique_id: String) {
        let service = self.clone();
        tokio::spawn(async move {
            let result = service.database.get_sticker_file_by_sticker_id(&sticker_unique_id).await;
            let file = match result {
                Ok(None) => return,
                Ok(Some(analysis)) => analysis,
                Err(err) => {
                    tracing::error!("database error while getting file info: {err:?}");
                    return;
                }
            };
            let result = service.vector_db.find_missing_stickers(vec![file.id]).await;
            let missing = match result {
                Ok(missing) => missing,
                Err(err) => {
                    tracing::error!("vector database error while getting missing files: {err}");
                    return;
                }
            };
            if !missing.is_empty() {
                let result = service.analyze_sticker(sticker_unique_id).await;
                report_periodic_task_error(result);
            }
        }.instrument(tracing::info_span!(parent: tracing::Span::none(), "analyze_sticker_background_task")));
    }

    #[tracing::instrument(skip(self, sticker))]
    async fn fetch_sticker_and_save_to_db(
        &self,
        sticker: teloxide::types::Sticker,
        set_name: String,
    ) -> Result<(), InternalError> {
        let emoji = sticker.emoji.map(|e| Emoji::new_from_string_single(&e));

        let (buf, file) = fetch_sticker_file(sticker.file.id.clone(), self.bot.clone()).await?;
        let thread_span =
            tracing::info_span!("spawn_blocking_calculate_sticker_file_hash").or_current();
        let file_hash = task::spawn_blocking(move || {
            thread_span.in_scope(|| {
                calculate_sticker_file_hash(buf)
            })
        })
        .await??;

        let canonical_file_hash = self.database.find_canonical_sticker_file_id(&file_hash).await?;

        self.database
            .create_file(
                &canonical_file_hash,
                sticker.thumb.map(|thumb| thumb.file.id),
                match sticker.format {
                    teloxide::types::StickerFormat::Raster => crate::database::StickerType::Static,
                    teloxide::types::StickerFormat::Animated => {
                        crate::database::StickerType::Animated
                    }
                    teloxide::types::StickerFormat::Video => crate::database::StickerType::Video,
                },
            )
            .await?;
        self.database
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

    #[tracing::instrument(skip(self, set), fields(set.name = set.name))]
    async fn fetch_sticker_set_and_save_to_db(
        &self,
        set: teloxide::types::StickerSet,
        user_id: Option<UserId>,
        numeric_set_id: Option<i64>,
    ) -> Result<(), InternalError> {
        // TODO: result should be how many stickers were added/removed/updated

        self.database
            .upsert_sticker_set_with_title(&set.name, &set.title, user_id.map(|id| id.0 as i64))
            .await?;

        if self.database
            .get_sticker_set_by_id(&set.name)
            .await?
            .is_some_and(|set| set.created_by_user_id.is_none())
        {
            let numeric_set_id = match numeric_set_id {
                Some(numeric_set_id) => Some(numeric_set_id),
                None => self.tg_service
                    .get_sticker_pack_id(set.name.clone())
                    .await
                    .map(|r| r.telegram_pack_id),
            };
            if let Some(telegram_pack_id) = numeric_set_id {
                let creator_id = decode_sticker_set_id(telegram_pack_id)?.owner_id;
                self.database
                    .upsert_sticker_set_with_creator(
                        &set.name,
                        creator_id,
                        user_id.map(|id| id.0 as i64),
                    )
                    .await?;
            }
        }

        let saved_stickers = self.database.get_all_stickers_in_set(&set.name).await?;

        let re = Regex::new(r"(\W|^)@([_a-zA-Z0-9]+)\b").expect("static regex to compile");
        for (_, [_, username]) in re.captures_iter(&set.title).map(|c| c.extract()) {
            self.database.add_username(username).await?;
        }

        let all_saved_sticker_file_hashes = saved_stickers
            .iter()
            .map(|s| s.sticker_file_id.to_string())
            .collect_vec();
        let missing_file_hashes = self.vector_db
            .find_missing_stickers(all_saved_sticker_file_hashes)
            .await?;

        let missing_sticker_ids = self.database
            .get_some_sticker_ids_for_sticker_file_ids(missing_file_hashes)
            .await?;
        for missing_sticker_id in missing_sticker_ids {
            self.analyze_sticker(
                missing_sticker_id.sticker_id,
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

        if self.config.is_readonly {
            return Ok(());
        }

        // todo: tag animated?
        for sticker in stickers_not_in_database_yet {
            let result = self.fetch_sticker_and_save_to_db(
                sticker.clone(),
                set.name.clone(),
            )
            .await;

            match result {
                Err(InternalError::Teloxide(teloxide::RequestError::Api(api_err)))
                    if is_wrong_file_id_error(&api_err) =>
                {
                    tracing::warn!("invalid file_id for a sticker, continuing");
                }
                Err(InternalError::Database(DatabaseError::TryingToInsertRemovedSticker)) => {
                    tracing::info!("trying to insert removed sticker")
                }
                Err(other) => {
                    if other.is_timeout_error() {
                        tracing::warn!("sticker fetch timed out, continuing");
                    } else {
                        return Err(other);
                    }
                }
                Ok(()) => {}
            }

            self.analyze_sticker(
                sticker.file.unique_id.clone(),
            )
            .await?;
        }
        for sticker in saved_stickers.clone() {
            let Some(s) = set.stickers.iter().find(|s| s.file.unique_id == sticker.id) else {
                continue;
            };
            if s.file.id != sticker.telegram_file_identifier {
                // TODO: might be updated too frequently
                self.database
                    .update_sticker(sticker.id, s.file.id.clone())
                    .await?;
            }
        }

        let deleted_stickers = saved_stickers
            .iter()
            .filter(|s| !set.stickers.iter().any(|s2| s2.file.unique_id == s.id))
            .collect_vec();

        for sticker in deleted_stickers {
            self.database.delete_sticker(&sticker.id).await?; // only deletes the sticker, not the file
            // TODO: periodically clean up no longer referenced files
        }

        for sticker in set.stickers.iter().filter(|s| s.is_raster()) {
            let result = automerge(
                &sticker.file.unique_id,
                self.database.clone(),
                self.vector_db.clone(),
                self.bot.clone(),
            )
            .await; // TODO: this might happen too frequently
            match result {
                Err(InternalError::UnexpectedNone { type_name }) => {
                    tracing::warn!("a {type_name} is unexpectedly none");
                }
                other => other?,
            }
        }
        
        self.possibly_auto_ban_set(&set.name, set.stickers.len()).await?;

        self.database.update_last_fetched(set.name.clone()).await?;

        Ok(())
    }
    
#[tracing::instrument(skip(self), err(Debug))]
 async fn analyze_sticker(
     &self,
    sticker_unique_id: String,
) -> Result<bool, InternalError> {
    let file_info = self.database
        .get_sticker_file_by_sticker_id(&sticker_unique_id)
        .await?;
    let Some(file_info) = file_info else {
        return Ok(false);
    };
    let sticker = self.database
        .get_sticker_by_id(&sticker_unique_id)
        .await?
        .required()?;
    let buf = if file_info.sticker_type == StickerType::Static {
        let (buf, _) =
            fetch_sticker_file(sticker.telegram_file_identifier.clone(), self.bot.clone()).await?; // this should always be an image
        buf
    } else {
        let Some(thumbnail_file_id) = file_info.thumbnail_file_id else {
            return Ok(false);
        };

        let (buf, _) = fetch_sticker_file(thumbnail_file_id.clone(), self.bot.clone()).await?; // this should always be an image
        buf
    };

    let buf_2 = buf.clone();
    let histogram = tokio::task::spawn_blocking(move || calculate_color_histogram(buf)).await??;
    let embedding = image_to_clip_embedding(buf_2, self.config.inference_url.clone()).await?;
    
    if self.possibly_auto_ban_sticker(embedding.clone(), &sticker_unique_id, &sticker.sticker_set_id).await? {
        return Ok(false);
    }

    self.vector_db
        .insert_sticker(embedding, histogram.into(), file_info.id.clone())
        .await?;

    return Ok(true);
}

#[tracing::instrument(skip(self), err(Debug))]
async fn possibly_auto_ban_set(&self, sticker_set_id: &str, set_sticker_count: usize) -> Result<bool, InternalError> {
    let banned_sticker_count = self.database.get_banned_sticker_count_for_set_id(sticker_set_id).await?;
    if banned_sticker_count > 10 || banned_sticker_count as f32 > set_sticker_count as f32 * 0.3 {
        // more than 10 stickers banned or set consists of more than 30% of banned stickers
        self.ban_sticker_set(sticker_set_id).await?;
        return Ok(true)
    }
    Ok(false)
}

/// the given sticker might exist in the main sticker table or not
#[tracing::instrument(skip(self, clip_vector), err(Debug))]
pub async fn possibly_auto_ban_sticker(&self, clip_vector: Vec<f32>, sticker_id: &str, sticker_set_id: &str) -> Result<bool, InternalError> {
    // do not auto ban already tagged stickers
    let has_tags = !self.database.get_sticker_tags(sticker_id).await?.is_empty();
    if has_tags {
        return Ok(false);
    }
    let matches = self.vector_db.find_banned_stickers_given_vector(clip_vector.clone()).await?;
    for m in matches {
        if let Some(threshold) = self.database.get_banned_sticker_max_match_distance(&m.file_hash).await? {
            if m.score >= threshold {
                let reduced_threshold = threshold + ((1.0 - threshold) / 2.0).max(0.0);
                tracing::info!(%reduced_threshold, %sticker_id, "auto banning sticker");
                self.ban_sticker(sticker_id, reduced_threshold, crate::database::BanReason::Automatic).await?;
                return Ok(true);
            }
            if m.score > 0.8 {
                let score_banned_sticker = m.score;
                // TODO: get rid of the magic number; goal is to not call this too often; also move the vector db call outside the loop?

                // if the sticker under consideration for a ban matches a banned sticker closer than 
                // the best match from a different set, also ban
                let best_regular_matches = self.vector_db.find_stickers_given_vector(clip_vector.clone(), 10, 0).await?;
                for m in best_regular_matches {
                    let score_non_banned_sticker = m.score;
                    if let Some(matched_sticker) = self.database.get_some_sticker_by_file_id(&m.file_hash).await? {
                        let has_tags = !self.database.get_sticker_tags_by_file_id(&m.file_hash).await?.is_empty();
                        if matched_sticker.sticker_set_id != sticker_set_id || has_tags {
                            // only consider stickers from other sets
                            // or consider stickers from same set if they are already tagged
                            if score_banned_sticker > score_non_banned_sticker {
                                tracing::info!(%score_banned_sticker, %score_non_banned_sticker, %sticker_id, %m.file_hash, "auto banning sticker");
                                self.ban_sticker(sticker_id, 0.95, crate::database::BanReason::Automatic).await?;
                                // TODO: get rid of magic number
                                return Ok(true);
                            }
                            break;
                        }
                    } else {
                        tracing::warn!(%m.file_hash, "could not find sticker");
                    }
                }
            }
        }
    }
    Ok(false)
}



    
    /// ban set and record who is to blame for adding it
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn ban_sticker_set(&self, set_id: &str) -> Result<(), DatabaseError> {
        let set = self.database.get_sticker_set_by_id(set_id).await?;
        self.database.delete_sticker_set(set_id).await?;
        self.database
            .ban_set(set_id, set.and_then(|set| set.added_by_user_id))
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn unban_sticker_set(&self, set_id: &str) -> Result<(), DatabaseError> {
        let original_adder = self.database.unban_set(&set_id).await?;
        self.database
            .upsert_sticker_set(&set_id, original_adder)
            .await?;
        self.queue_sticker_set_import(
                &set_id,
                true,
                original_adder.map(|id| UserId(id as u64)),
                None,
            )
            .await;
        Ok(())
    }

    /// ban a sticker that already exists in the database
    pub async fn ban_sticker(
        &self,
        sticker_id: &str,
        clip_max_match_distance: f32,
        ban_reason: BanReason,
    ) -> Result<(), InternalError> {
        let Some(sticker_file) = self
            .database
            .get_sticker_file_by_sticker_id(sticker_id)
            .await?
        else {
            tracing::error!("missing sticker file");
            return Err(InternalError::OperationFailed);
        };
        let Some(sticker) = self.database.get_sticker_by_id(sticker_id).await? else {
            tracing::error!("missing sticker file");
            return Err(InternalError::OperationFailed);
        };

        let clip_vector = self
            .vector_db
            .get_sticker_clip_vector(sticker.sticker_file_id.clone())
            .await?;
        self.vector_db
            .insert_banned_sticker(clip_vector, sticker.sticker_file_id.clone())
            .await?;
        self.vector_db.delete_stickers(vec![sticker.sticker_file_id.clone()]).await?;

        self.database.delete_sticker(sticker_id).await?;
        self.database
            .ban_sticker(
                sticker_id,
                &sticker.telegram_file_identifier,
                &sticker.sticker_set_id,
                &sticker.sticker_file_id,
                &sticker_file.thumbnail_file_id,
                sticker_file.sticker_type,
                clip_max_match_distance,
                ban_reason,
            )
            .await?;
        Ok(())
    }

    pub async fn unban_sticker(&self, sticker_id: &str) -> Result<(), InternalError> {
        let (set_id, sticker_file_id) = self.database.unban_sticker(sticker_id).await?;
        self.vector_db
            .delete_banned_stickers(vec![sticker_file_id.clone()])
            .await?;
        self
            .queue_sticker_set_import(
                &set_id,
                true,
                None,
                None,
            )
            .await;
        Ok(())
    }
}
