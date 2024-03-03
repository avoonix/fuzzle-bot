use std::io::Cursor;

use image::{GenericImageView, Rgba, RgbaImage};
use itertools::Itertools;
use palette::num::SaturatingAdd;

use crate::{
    bot::{Bot, BotError},
    database::{Database, FileAnalysis},
    web::shared::{StickerMergeInfo, StickerMergeInfos, StickerSetDto},
};

use super::{cosine_similarity, download::fetch_sticker_file, vec_u8_to_f32, ModelEmbedding};

fn get_f32_vecs(analysis: FileAnalysis) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let embedding = ModelEmbedding::from(analysis.embedding.unwrap());
    let embedding: Vec<f32> = embedding.into();

    let histogram = vec_u8_to_f32(analysis.histogram.unwrap());

    let visual_hash = vec_u8_to_f32(analysis.visual_hash.unwrap());

    (embedding, histogram, visual_hash)
}

fn get_similarites(analysis_a: FileAnalysis, analysis_b: FileAnalysis) -> (f32, f32, f32) {
    let (embedding_a, histogram_a, visual_hash_a) = get_f32_vecs(analysis_a);
    let (embedding_b, histogram_b, visual_hash_b) = get_f32_vecs(analysis_b);

    (
        cosine_similarity(embedding_a, embedding_b),
        cosine_similarity(histogram_a, histogram_b),
        cosine_similarity(visual_hash_a, visual_hash_b),
    )
}

pub async fn get_merge_infos(
    sticker_id_a: String,
    sticker_id_b: String,
    database: Database,
) -> Result<StickerMergeInfos, BotError> {
    let analysis_a = database
        .get_analysis_for_sticker_id(sticker_id_a.clone())
        .await?
        .unwrap();
    let analysis_b = database
        .get_analysis_for_sticker_id(sticker_id_b.clone())
        .await?
        .unwrap();
    let sticker_a = database.get_sticker(sticker_id_a.clone()).await?.unwrap();
    let sticker_b = database.get_sticker(sticker_id_b.clone()).await?.unwrap();
    let sticker_set_a = database.get_sticker_set(sticker_a.set_id).await?.unwrap();
    let sticker_set_b = database.get_sticker_set(sticker_b.set_id).await?.unwrap();
    let sticker_file_a = database
        .get_file_info(sticker_id_a.clone())
        .await?
        .unwrap();
    let sticker_file_b = database
        .get_file_info(sticker_id_b.clone())
        .await?
        .unwrap();
    let sticker_sets_a = database.get_sets_containing_sticker(sticker_id_a.clone()).await?;
    let sticker_sets_b = database.get_sets_containing_sticker(sticker_id_b.clone()).await?;

    let sticker_sets_a = sticker_sets_a.into_iter().map(|set| StickerSetDto { id: set.id, title: set.title, }).collect_vec();
    let sticker_sets_b = sticker_sets_b.into_iter().map(|set| StickerSetDto { id: set.id, title: set.title, }).collect_vec();

    let can_be_merged = !sticker_set_a.is_animated && !sticker_set_b.is_animated;

    let (embedding_similarity, histogram_similarity, visual_hash_similarity) =
        get_similarites(analysis_a, analysis_b);

    let now = chrono::Utc::now().naive_utc();
    let days_known_a = (now - sticker_file_a.created_at).num_days();
    let days_known_b = (now - sticker_file_b.created_at).num_days();

    let sticker_a = StickerMergeInfo {
        days_known: days_known_a,
        instance_count: sticker_file_a.sticker_count.unwrap_or_default() as i64,
        sticker_sets: sticker_sets_a,
    };
    let sticker_b = StickerMergeInfo {
        days_known: days_known_b,
        instance_count: sticker_file_b.sticker_count.unwrap_or_default() as i64,
        sticker_sets: sticker_sets_b,
    };

    Ok(StickerMergeInfos {
        can_be_merged,
        embedding_similarity,
        histogram_similarity,
        visual_hash_similarity,
        sticker_a,
        sticker_b,
    })
}

pub async fn generate_merge_image(
    sticker_id_a: String,
    sticker_id_b: String,
    database: Database,
    bot: Bot,
) -> anyhow::Result<Vec<u8>> {
    let sticker_a = database
        .get_sticker(sticker_id_a)
        .await?
        .ok_or(anyhow::anyhow!("sticker not found"))?;
    let sticker_b = database
        .get_sticker(sticker_id_b)
        .await?
        .ok_or(anyhow::anyhow!("sticker not found"))?;
    let (buf_a, _) = fetch_sticker_file(sticker_a.file_id.clone(), bot.clone()).await?;
    let (buf_b, _) = fetch_sticker_file(sticker_b.file_id.clone(), bot.clone()).await?; // TODO: can do in parallel

    let dynamic_image_a = image::load_from_memory(&buf_a)?;
    let dynamic_image_a =
        dynamic_image_a.resize_exact(512, 512, image::imageops::FilterType::Gaussian);
    let dynamic_image_b = image::load_from_memory(&buf_b)?;
    let dynamic_image_b =
        dynamic_image_b.resize_exact(512, 512, image::imageops::FilterType::Gaussian);

    let mut img = RgbaImage::new(512 * 4, 512);

    for x in 0..img.width() {
        for y in 0..img.height() {
            let pixel_color = if x < 512 {
                // original image a
                dynamic_image_a.get_pixel(x, y)
            } else if x < 1024 {
                let x = x - 512;
                // difference mask
                let a = dynamic_image_a.get_pixel(x, y);
                let b = dynamic_image_b.get_pixel(x, y);
                let diff = get_difference(a, b);
                if diff > 100 {
                    Rgba([255, 0, 0, 255])
                } else if diff > 80 {
                    Rgba([255, 200, 0, 255])
                } else if diff > 40 {
                    Rgba([150, 255, 0, 255])
                } else if diff > 10 {
                    Rgba([0, 255, 50, 255])
                } else if diff > 0 {
                    Rgba([0, 255, 255, 255])
                } else {
                    Rgba([0, 0, 0, 0])
                }
            } else if x < 1024 + 512 {
                let x = x - 1024;
                // colored difference
                let a = dynamic_image_a.get_pixel(x, y);
                let b = dynamic_image_b.get_pixel(x, y);
                let diff = get_difference(a, b.clone());
                if diff > 0 {
                    Rgba([255 - b.0[0], 255 - b.0[1], 255 - b.0[2], diff])
                } else {
                    Rgba([255, 255, 255, 255])
                }
            } else {
                let x = x - 1024 - 512;
                // original image b
                dynamic_image_b.get_pixel(x, y)
            };
            img.put_pixel(x, y, pixel_color);
        }
    }
    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut Cursor::new(&mut bytes), image::ImageOutputFormat::WebP)?;
    Ok(bytes)
}

fn get_difference(a: Rgba<u8>, b: Rgba<u8>) -> u8 {
    let diff_r = a.0[0].abs_diff(b.0[0]);
    let diff_g = a.0[1].abs_diff(b.0[1]);
    let diff_b = a.0[2].abs_diff(b.0[2]);
    let diff_a = a.0[3].abs_diff(b.0[3]);
    if a.0[3] == 0 && b.0[3] == 0 {
        0 // color difference does not matter if both images are fully transparent
    } else {
        diff_r
            .saturating_add(diff_b)
            .saturating_add(diff_g)
            .saturating_add(diff_a)
    }
}

// pub async fn merge_stickers() -> Result<(), BotError> {
// // actual merge (in a different file)
// // begin transaction
// // insert file hashes in a "merged_stickers" table
// // merge all tags
// // delete one of the sticker files
// // end transaction
// }
