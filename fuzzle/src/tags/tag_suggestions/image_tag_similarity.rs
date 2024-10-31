use std::{collections::HashMap, sync::Arc};

use itertools::Itertools;

use crate::{background_tasks::{TagManagerService}, bot::{InternalError}, database::Database, qdrant::VectorDatabase};

use super::ScoredTagSuggestion;

#[tracing::instrument(skip(database, vector_db, tag_manager))]
pub async fn suggest_closest_tags(
    database: &Database,
    vector_db: &VectorDatabase,
    tag_manager: TagManagerService,
    file_hash: &str,
) -> Result<Vec<ScoredTagSuggestion>, InternalError> {
    let Some(result) = vector_db.recommend_tags(file_hash).await? else {return Ok(vec![])}; // TODO: this might fail if the sticker is not indexed yet
    Ok(convert_vectordb_recommended_tags_to_suggestions(
        result,
        tag_manager,
    ).await?)
}

pub async fn convert_vectordb_recommended_tags_to_suggestions(
    result: Vec<String>,
    tag_manager: TagManagerService,
) -> Result<Vec<ScoredTagSuggestion>, InternalError> {
    let mut tags: HashMap<String, f64> = HashMap::new();
    let mut score = 1.0;
    for tag_or_alias in result {
        let Some(tag) = tag_manager.closest_matching_tag(&tag_or_alias).await else { continue; };
        tags.entry(tag)
            .and_modify(|s| *s = s.max(score))
            .or_insert(score);
        score *= 0.9;
    }
    Ok(tags.into_iter()
        .map(|(tag, score)| ScoredTagSuggestion { tag, score })
        .collect_vec())
}
