#[cfg(feature = "ssr")]
mod histogram;
mod measures;
#[cfg(feature = "ssr")]
mod util;
#[cfg(feature = "ssr")]
mod visual_hash;

pub use measures::{Match, Measures, TopMatches};

#[cfg(feature = "ssr")]
pub use histogram::{calculate_color_histogram, create_historgram_image, Histogram};
#[cfg(feature = "ssr")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
pub use visual_hash::{calculate_visual_hash, create_visual_hash_image, VisualHash};

#[cfg(feature = "ssr")]
use log::{info, warn};

#[cfg(feature = "ssr")]
use crate::{
    bot::{Bot, BotError},
    database::{Database, SavedSticker},
    sticker::{analysis::util::cosine_similarity, fetch_possibly_cached_sticker_file},
    Paths,
};

#[cfg(feature = "ssr")]
pub async fn compute_similar(database: Database, sticker_id: String) -> Result<Measures, BotError> {
    let sticker = database
        .get_analysis_for_sticker_id(sticker_id)
        .await?
        .ok_or(anyhow::anyhow!("sticker analysis not found"))?; // TODO: dont use anyhow
    let query_histogram = sticker
        .histogram
        .ok_or(anyhow::anyhow!("histogram not found"))?; // TODO: no anyhow
    let query_visual_hash = sticker
        .visual_hash
        .ok_or(anyhow::anyhow!("visual_hash not found"))?; // TODO: no anyhow
    let analysis = database.get_analysis_for_all_stickers().await?;

    tokio::task::spawn_blocking(move || {
        let mut measures = Measures::new(100, 0.3, 0.2); // TODO: dont hardcode here
        for analysis in analysis {
            let Some(sticker_id) = analysis.sticker_id else {
                continue;
            };
            if let Some(hist) = analysis.histogram {
                let distance = 1.0 - cosine_similarity(query_histogram.clone(), hist.clone());
                measures.histogram_cosine.push(distance, sticker_id.clone());
            };
            if let Some(hash) = analysis.visual_hash {
                let distance = 1.0 - cosine_similarity(query_visual_hash.clone(), hash.clone());
                measures
                    .visual_hash_cosine
                    .push(distance, sticker_id.clone());
            };
        }

        Ok(measures)
    }).await?
}

#[cfg(feature = "ssr")]
pub async fn analyze_n_stickers(
    database: Database,
    bot: Bot,
    n: i64,
    paths: Paths,
) -> Result<(), BotError> {
    let analysis = database
        .get_n_stickers_with_missing_analysis(n)
        .await
        .unwrap();
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
            info!("computed visual hash for sticker {}", analysis.id);
        }

        if analysis.histogram.is_none() {
            let histogram =
                tokio::task::spawn_blocking(move || calculate_color_histogram(buf)).await??;
            database
                .update_histogram(analysis.id.clone(), histogram.into())
                .await?;
            info!("computed histogram for sticker {}", analysis.id);
        }
    }

    Ok(())
}
