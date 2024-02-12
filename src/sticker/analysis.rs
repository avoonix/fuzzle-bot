use log::{info, warn};

use crate::{
    bot::{Bot, BotError},
    database::Database,
    sticker::fetch_possibly_cached_sticker_file,
    Paths,
};

use super::{calculate_visual_hash, download::FileKind, fetch_sticker_file};

pub async fn analyze_n_stickers(
    database: Database,
    bot: Bot,
    n: i64,
    paths: Paths,
) -> Result<(), BotError> {
    let analysis = database
        .get_n_stickers_with_missing_analysis(n)
        .await
        .unwrap(); // TODO: if there is an error (eg invalid file type) -> infinite loop!!!
    for analysis in analysis {
        let Some(thumbnail_file_id) = analysis.thumbnail_file_id else {
            warn!("sticker {} does not have a thumbnail", analysis.id);
            continue;
        };

        let buf =
            fetch_possibly_cached_sticker_file(thumbnail_file_id, bot.clone(), paths.image_cache())
                .await?;
        let file_kind = FileKind::Image; // TODO: determine file kind properly (although this should always be an image)
        let visual_hash =
            tokio::task::spawn_blocking(move || calculate_visual_hash(buf, file_kind)).await??;
        let Some(visual_hash) = visual_hash else {
            warn!("could not compute visual hash for sticker {}", analysis.id);
            continue;
        };

        database
            .update_visual_hash(analysis.id.clone(), visual_hash)
            .await?;

        info!("computed visual hash for sticker {}", analysis.id);
    }

    Ok(())
}
