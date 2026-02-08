mod histogram;
mod measures;
mod util;

pub use histogram::{calculate_color_histogram, create_historgram_image};
use itertools::Either;
pub use measures::{Match, Measures};
use qdrant_client::qdrant::Vector;

use crate::{
    Config, bot::{
        Bot, BotError, InternalError, RequestContext, UserError, report_bot_error, report_internal_error_result, report_periodic_task_error
    }, database::{Database, StickerType}, inference::{image_to_clip_embedding, text_to_clip_embedding}, util::Required
};

use crate::inline::SimilarityAspect;

use std::sync::Arc;

pub use util::{cosine_similarity, vec_u8_to_f32};

use crate::qdrant::StickerMatch;
use crate::qdrant::VectorDatabase;

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
    Ok((
        resolve_file_hashes_to_sticker_ids_and_clean_up_unreferenced_files(database, vector_db, file_hashes)
            .await?,
        len,
    ))
}

#[tracing::instrument(skip(database))]
pub async fn resolve_file_hashes_to_sticker_ids_and_clean_up_unreferenced_files(
    database: Database,
    vector_db: VectorDatabase,
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
    let (result, unreferenced_file_ids): (Vec<_>, Vec<_>) =
        file_hashes.into_iter().partition_map(|file_id| {
            result
                .iter()
                .find(|r| r.sticker_file_id == file_id.file_hash)
                .map(|r| {
                    Either::Left(Match {
                        distance: file_id.score,
                        sticker_id: r.sticker_id.clone(),
                    })
                })
                .unwrap_or_else(|| Either::Right(file_id.file_hash))
        });
    if !unreferenced_file_ids.is_empty() {
        tokio::spawn(async move {
            let result = vector_db.delete_stickers(unreferenced_file_ids).await;
            match result {
                Ok(_) => {}
                Err(err) => tracing::error!("unreferenced file cleanup error: {err:?}"),
            }
        });
    }

    Ok(result)
}
