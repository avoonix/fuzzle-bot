use itertools::Itertools;

use crate::{bot::InternalError, database::Database, qdrant::VectorDatabase};

use super::ScoredTagSuggestion;

#[tracing::instrument(skip(database))]
pub async fn suggest_tags_from_similar_stickers(
    database: &Database,
    vector_db: &VectorDatabase,
    file_hash: &str,
    score_threshold: f32,
    limit: u64,
) -> Result<Vec<ScoredTagSuggestion>, InternalError> {
    let result = vector_db.find_similar_stickers(&[file_hash.to_string()], &[], crate::inline::SimilarityAspect::Embedding, score_threshold, limit, 0).await;
    let Some(result) = (match result {
        Ok(res) => res,
        Err(err) => {
            tracing::error!("vector db error: {err}");
            return Ok(vec![]);
        }
    }) else {return Ok(vec![])};
    let result = result.into_iter().map(|r| r.file_hash).collect_vec();
    let result = database.get_some_sticker_ids_for_sticker_file_ids(result).await?;
    let result = result.into_iter().map(|a| a.sticker_id).collect_vec(); // TODO: take into account the order of the matches?
    get_all_tags_from_stickers(result, database.clone()).await
}

async fn get_all_tags_from_stickers(
    sticker_unique_ids: Vec<String>,
    database: Database,
) -> Result<Vec<ScoredTagSuggestion>, InternalError> {
    let tag_res = database.get_multiple_sticker_tags(sticker_unique_ids).await?;
    let max = tag_res.iter().map(|r| r.1).max().unwrap_or(1);
    let res = tag_res.into_iter().map(|r| ScoredTagSuggestion { tag: r.0, score: r.1 as f64 / max as f64 }).collect_vec();
    Ok(res)
}
