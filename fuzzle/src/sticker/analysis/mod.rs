mod histogram;
mod measures;
mod util;

pub use histogram::{calculate_color_histogram, create_historgram_image};
pub use measures::{Match, Measures};

use crate::{
    background_tasks::BackgroundTaskExt, bot::{report_bot_error, report_internal_error_result, report_periodic_task_error, Bot, BotError, InternalError, RequestContext, UserError}, database::{Database, StickerType}, inference::{image_to_clip_embedding, text_to_clip_embedding}, util::Required, Config
};

use crate::inline::SimilarityAspect;

use std::sync::Arc;

pub use util::{cosine_similarity, vec_u8_to_f32};

use crate::qdrant::StickerMatch;
use crate::qdrant::VectorDatabase;

use super::{import_all_stickers_from_set, import_individual_sticker_and_queue_set};

pub async fn find_with_text_embedding(
    database: Database,
    text: String,
    vector_db: VectorDatabase,
    config: Arc<Config>,
    limit: usize,
    offset: usize,
) -> Result<(Vec<Match>, usize), BotError> {
    let query_embedding = text_to_clip_embedding(text, config.inference_url.clone()).await?;

    let file_hashes = vector_db
        .find_stickers_given_vector(query_embedding.into(), limit as u64, offset as u64)
        .await?;
    let len = file_hashes.len();
    Ok((with_sticker_id(database, file_hashes).await?, len))
}

#[tracing::instrument(skip(database))]
pub async fn with_sticker_id(
    database: Database,
    file_hashes: Vec<StickerMatch>,
) -> Result<Vec<Match>, BotError> {
    let file_hashes_ = file_hashes
        .iter()
        .map(|r| r.file_hash.clone())
        .collect_vec();
    // TODO: add score

    use itertools::Itertools;

    let result = database
        .get_some_sticker_ids_for_sticker_file_ids(file_hashes_.clone())
        .await?;
    let result = file_hashes
        .into_iter()
        .filter_map(|file_id| {
            result
                .iter()
                .find(|r| r.sticker_file_id == file_id.file_hash)
                .map(|r| Match {
                    distance: file_id.score,
                    sticker_id: r.sticker_id.clone(),
                })
        })
        .collect_vec();
    // TODO: if there are file hashes that don't have any stickers -> delete the file hashes

    Ok(result)
}

#[tracing::instrument(skip(request_context))]
pub async fn compute_similar(
    request_context: RequestContext,
    sticker_id: String,
    aspect: SimilarityAspect,
    limit: u64,
    offset: u64,
) -> Result<(Vec<Match>, usize), BotError> {
    let sticker = request_context.database.get_sticker_by_id(&sticker_id).await?.required()?;
    let score_threshold = 0.0;

    // let result = vector_db.find_similar_stickers(query_embedding.clone().into()).await?;
    let file_hashes = request_context.vector_db
        .find_similar_stickers(&[sticker.sticker_file_id.clone()], &[], aspect, score_threshold, limit, offset)
        .await?;
    let file_hashes = match file_hashes {
    Some(hashes) => hashes,
    None => {
        request_context.process_sticker_set(sticker.sticker_set_id, false).await; // dispatch in background - otherwise the query would take too long if the set is large
        return Err(UserError::VectorNotFound.into());
    }};

    let len = file_hashes.len();
    Ok((with_sticker_id(request_context.database.clone(), file_hashes).await?, len))
}

#[tracing::instrument(skip(database, bot, config, vector_db), err(Debug))]
pub async fn analyze_sticker(
    sticker_unique_id: String,
    database: Database,
    bot: Bot,
    config: Arc<Config>,
    vector_db: VectorDatabase,
) -> Result<bool, InternalError> {
    use super::download::fetch_sticker_file;

    let file_info = database
        .get_sticker_file_by_sticker_id(&sticker_unique_id)
        .await?;
    let Some(file_info) = file_info else {
        return Ok(false);
    };
    let buf = if file_info.sticker_type == StickerType::Static {
        let sticker = database
            .get_sticker_by_id(&sticker_unique_id)
            .await?
            .required()?;

        let (buf, _) =
            fetch_sticker_file(sticker.telegram_file_identifier.clone(), bot.clone()).await?; // this should always be an image
        buf
    } else {
        let Some(thumbnail_file_id) = file_info.thumbnail_file_id else {
            return Ok(false);
        };

        let (buf, _) = fetch_sticker_file(thumbnail_file_id.clone(), bot.clone()).await?; // this should always be an image
        buf
    };

    let buf_2 = buf.clone();
    let histogram = tokio::task::spawn_blocking(move || calculate_color_histogram(buf)).await??;
    let embedding = image_to_clip_embedding(buf_2, config.inference_url.clone()).await?;
    vector_db
        .insert_sticker(embedding, histogram.into(), file_info.id.clone())
        .await?;
    return Ok(true);
}
