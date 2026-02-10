use std::{collections::HashMap, io::Cursor};

use duplicate_detector::image_differences_within_thresholds;
use image::{GenericImageView, Rgba, RgbaImage};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{
    bot::{Bot, InternalError},
    database::{
Database, MergeStatus, StickerType
    },
    qdrant::VectorDatabase, util::Required,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerSetDto {
    pub id: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerMergeInfos {
    pub can_be_merged: bool,
    pub embedding_similarity: f32,
    pub histogram_similarity: f32,
    pub sticker_a: StickerMergeInfo,
    pub sticker_b: StickerMergeInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerMergeInfo {
    pub days_known: i64,
    pub instance_count: i64,
    pub sticker_sets: Vec<StickerSetDto>,
}

use super::download::fetch_sticker_file;

#[tracing::instrument(skip(database, bot))]
pub async fn generate_merge_image(
    sticker_id_a: &str,
    sticker_id_b: &str,
    database: Database,
    bot: Bot,
) -> Result<Vec<u8>, InternalError> {
    let buf_a = get_sticker_file(database.clone(), bot.clone(), sticker_id_a).await?;
    let buf_b = get_sticker_file(database.clone(), bot.clone(), sticker_id_b).await?; // TODO: can do in parallel

    let image_a = image::load_from_memory(&buf_a)?;
    let image_b = image::load_from_memory(&buf_b)?;

    let thread_span =
        tracing::info_span!("spawn_blocking_generate_merge_image").or_current();
    let img = tokio::task::spawn_blocking(move || {
        thread_span.in_scope(|| {
            duplicate_detector::generate_merge_image(image_a, image_b, &Default::default())
        })
    })
    .await?;

    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)?;
    Ok(bytes)
}

#[tracing::instrument(skip(database, bot), err(Debug))]
async fn get_sticker_file(
    database: Database,
    bot: Bot,
    sticker_id: &str,
) -> Result<Vec<u8>, InternalError> {
    let sticker_a = database.get_sticker_by_id(sticker_id).await?.required()?;
    let (buf, _) = fetch_sticker_file(sticker_a.telegram_file_identifier.clone(), bot.clone()).await?;
    Ok(buf)
}

#[tracing::instrument(skip(database, vector_db, bot))]
pub async fn automerge(
    sticker_id: &str,
    database: Database,
    vector_db: VectorDatabase,
    bot: Bot,
) -> Result<(), InternalError> {
    let sticker = database.get_sticker_by_id(sticker_id).await?.required()?;
    let similar_sticker_file_hashes_1 = vector_db
        .find_similar_stickers(
            &[sticker.sticker_file_id.clone()],
            &[],
            crate::inline::SimilarityAspect::Embedding,
            0.96,
            10,
            0,
        )
        .await?;
    let similar_sticker_file_hashes_2 = vector_db
        .find_similar_stickers(
            &[sticker.sticker_file_id.clone()],
            &[],
            crate::inline::SimilarityAspect::Color,
            0.98,
            10,
            0,
        )
        .await?;
    let Some((similar_sticker_file_hashes_1, similar_sticker_file_hashes_2)) =
        similar_sticker_file_hashes_1.zip(similar_sticker_file_hashes_2)
    else {
        return Ok(());
    };
    let already_considered = database
        .get_all_merge_candidate_file_ids(&sticker.sticker_file_id)
        .await?;
    let similar_sticker_file_hashes_1 = similar_sticker_file_hashes_1
        .into_iter()
        .filter(|sticker| !already_considered.contains(&sticker.file_hash))
        .collect_vec();
    let similar_sticker_file_hashes_2 = similar_sticker_file_hashes_2
        .into_iter()
        .filter(|sticker| !already_considered.contains(&sticker.file_hash))
        .collect_vec();
    let most_similar = similar_sticker_file_hashes_1
        .into_iter()
        .filter_map(|match_1| {
            similar_sticker_file_hashes_2
                .iter()
                .find(|m| m.file_hash == match_1.file_hash)
                .map(|match_2| {
                    (
                        match_1.file_hash,
                        harmonic_mean(vec![match_1.score, match_2.score]),
                    )
                })
        })
        .filter(|(_, m)| *m > 0.98)
        .sorted_by(|(_, m1), (_, m2)| m2.total_cmp(&m1))
        .map(|(file_hash, _)| file_hash)
        .collect_vec();
    let result = database
        .get_some_sticker_ids_for_sticker_file_ids(most_similar.clone())
        .await?;

    if result.is_empty() {
        return Ok(());
    }

    let buf_a = get_sticker_file(database.clone(), bot.clone(), sticker_id).await?;
    let Some(sticker_a_file) = database.get_sticker_file_by_sticker_id(sticker_id).await? else {
        return Ok(());
    };
    for file_hash in most_similar {
        let Some(sticker) = result.iter().find(|r| r.sticker_file_id == file_hash) else {
            continue;
        };
        let sticker_id_b = sticker.sticker_id.clone();
        let Some(sticker_b_file) = database.get_sticker_file_by_sticker_id(&sticker_id_b).await?
        else {
            continue;
        };
        if sticker_b_file.sticker_type != StickerType::Static || sticker_a_file.id == sticker_b_file.id {
            continue;
        }
        let buf_b = get_sticker_file(database.clone(), bot.clone(), &sticker_id_b).await?;
        let buf_a = buf_a.clone();
        

        let thread_span =
            tracing::info_span!("spawn_blocking_within_threshold").or_current();
        let within_thresholds = tokio::task::spawn_blocking(move || thread_span.in_scope(|| within_threshold(buf_a, buf_b))).await??;
        
        if within_thresholds {
            return determine_canonical_sticker_and_merge(
                sticker_id.to_string(),
                sticker_id_b,
                database,
            )
            .await;
        } else {
            database
                .add_or_modify_potential_merge(
                    &sticker_a_file.id,
                    &sticker_b_file.id,
                    MergeStatus::Queued,
                )
                .await?;
        }
    }
    Ok(())
}

#[tracing::instrument(skip(buf_a, buf_b), err(Debug))]
fn within_threshold(buf_a: Vec<u8>, buf_b: Vec<u8>) -> anyhow::Result<bool> {
    let cloned = buf_a.clone();
    let dynamic_image_a = image::load_from_memory(&buf_a)?;
    let dynamic_image_b = image::load_from_memory(&buf_b)?;
    Ok(image_differences_within_thresholds(
        &dynamic_image_a,
        &dynamic_image_b,
        &Default::default(),
    ))
}

#[tracing::instrument(skip(database))]
pub async fn determine_canonical_sticker_and_merge(
    sticker_id_a: String,
    sticker_id_b: String,
    database: Database,
) -> Result<(), InternalError> {
    let sticker_file_a = database
        .get_sticker_file_by_sticker_id(&sticker_id_a)
        .await?
        .required()?;
    let sticker_file_b = database
        .get_sticker_file_by_sticker_id(&sticker_id_b)
        .await?
        .required()?;
    let (canonical_file_id, duplicate_file_id) =
        if sticker_file_a.created_at < sticker_file_b.created_at {
            (&sticker_file_a.id, &sticker_file_b.id)
        } else {
            (&sticker_file_b.id, &sticker_file_a.id)
        };
    database
        .merge_stickers(canonical_file_id, duplicate_file_id, None)
        .await?;
    Ok(())
}

// TODO: unit tests
fn harmonic_mean(numbers: Vec<f32>) -> f32 {
    let len = numbers.len() as f32;
    let mut sum = 0.0;
    for num in numbers {
        if num == 0.0 {
            return 0.0;
        }
        sum += 1.0 / num;
    }
    if sum == 0.0 {
        0.0
    } else {
        len / sum
    }
}

// - after inserting new sticker in qdrant: check if inserted sticker is not animated + check if both histogram and embedding similarities are >.99
//     - if they are, download both full sticker images
//     - check if the dimensions are identical
//     - iterate over all pixels and check if all differences are below <the green color in the merge image diff tool> and also count the
//         number of pixels above that threshold - for now just output that number; then see how high that number for almost identical images is, and
//         allow merge if it is below a threshold

// pub async fn merge_stickers(canonical_sticker_id: String, duplicate_sticker_id: String) -> Result<(), BotError> {

// // begin transaction
// // insert file hashes in a "merged_stickers" table
// // merge all tags
// // delete one of the sticker files
// // end transaction
// }

// TODO: when creating stickers: check if sticker hash is in merged stickers; if it is, change the hash (and check again in loop)
