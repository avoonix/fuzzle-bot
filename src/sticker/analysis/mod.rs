#[cfg(feature = "ssr")]
mod histogram;
mod measures;
#[cfg(feature = "ssr")]
mod model;
#[cfg(feature = "ssr")]
mod tokenizer;
#[cfg(feature = "ssr")]
mod util;
#[cfg(feature = "ssr")]
mod visual_hash;

#[cfg(feature = "ssr")]
mod index;

#[cfg(feature = "ssr")]
pub use index::*;

pub use measures::{Match, Measures, TopMatches};

#[cfg(feature = "ssr")]
pub use histogram::{calculate_color_histogram, create_historgram_image};
#[cfg(feature = "ssr")]
pub use model::{EmbeddingError, ModelEmbedding};

#[cfg(feature = "ssr")]
pub use visual_hash::{calculate_visual_hash};

#[cfg(feature = "ssr")]
use log::{warn};

#[cfg(feature = "ssr")]
use crate::{
    bot::{Bot, BotError},
    database::{Database},
    sticker::fetch_possibly_cached_sticker_file,
    Paths,
};

#[cfg(feature = "ssr")]
use crate::background_tasks::AnalysisWorker;

#[cfg(feature = "ssr")]
use crate::inline::SimilarityAspect;

#[cfg(feature = "ssr")]
use std::sync::Arc;

#[cfg(feature = "ssr")]
pub use util::vec_u8_to_f32;

#[cfg(feature = "ssr")]
pub async fn find_with_text_embedding(
    database: Database,
    text: String,
    worker: AnalysisWorker,
    n: usize,
) -> Result<TopMatches, BotError> {
    use crate::sticker::analysis::{model::ModelEmbedding};

    let query_embedding = ModelEmbedding::from_text(&text)?;
    
    worker
        .retrieve(query_embedding.into(), n, SimilarityAspect::Embedding)
        .await
}

#[cfg(feature = "ssr")]
pub async fn compute_similar(
    database: Database,
    sticker_id: String,
    aspect: SimilarityAspect,
    worker: AnalysisWorker,
    n: usize,
) -> Result<TopMatches, BotError> {
    let sticker = database
        .get_analysis_for_sticker_id(sticker_id)
        .await?
        .ok_or(anyhow::anyhow!("sticker analysis not found"))?; // TODO: dont use anyhow
    match aspect {
        SimilarityAspect::Color => {
            let query_histogram = vec_u8_to_f32(
                sticker
                    .histogram
                    .ok_or(anyhow::anyhow!("histogram not found"))?,
            ); // TODO: no anyhow

            worker
                .retrieve(query_histogram, n, SimilarityAspect::Color)
                .await
        }
        SimilarityAspect::Embedding => {
            let query_embedding: ModelEmbedding = sticker
                .embedding
                .ok_or(anyhow::anyhow!("embedding not found"))?
                .into(); // TODO: no anyhow
            worker
                .retrieve(query_embedding.into(), n, SimilarityAspect::Embedding)
                .await
        }
        SimilarityAspect::Shape => {
            let query_visual_hash = vec_u8_to_f32(
                sticker
                    .visual_hash
                    .ok_or(anyhow::anyhow!("visual_hash not found"))?,
            ); // TODO: no anyhow
            worker
                .retrieve(query_visual_hash, n, SimilarityAspect::Shape)
                .await
        }
    }
}

#[cfg(feature = "ssr")]
pub async fn analyze_n_stickers(
    database: Database,
    bot: Bot,
    n: i64,
    paths: Arc<Paths>,
    worker: AnalysisWorker,
) -> Result<(), BotError> {
    let analysis = database
        .get_n_stickers_with_missing_analysis(n)
        .await?;
    let mut changed = false;
    for analysis in analysis {
        let Some(thumbnail_file_id) = analysis.thumbnail_file_id else {
            warn!("sticker {} does not have a thumbnail", analysis.id);
            continue;
        };

        let buf =
            fetch_possibly_cached_sticker_file(thumbnail_file_id, bot.clone(), paths.image_cache())
                .await?;
        // this should always be an image

        if analysis.visual_hash.is_none() {
            let buf = buf.clone();
            let visual_hash =
                tokio::task::spawn_blocking(move || calculate_visual_hash(buf)).await??;
            database
                .update_visual_hash(analysis.id.clone(), visual_hash.into())
                .await?;
            changed = true;
        }

        if analysis.histogram.is_none() {
            let buf = buf.clone();
            let histogram =
                tokio::task::spawn_blocking(move || calculate_color_histogram(buf)).await??;
            database
                .update_histogram(analysis.id.clone(), histogram.into())
                .await?;
            changed = true;
        }

        if analysis.embedding.is_none() {
            let embedding =
                tokio::task::spawn_blocking(move || ModelEmbedding::from_image_buf(buf)).await??;
            database
                .update_embedding(analysis.id.clone(), embedding.into())
                .await?;
            changed = true;
        }
    }

    if changed {
        worker.recompute().await?;
    }

    Ok(())
}
