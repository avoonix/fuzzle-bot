use std::{collections::HashMap, sync::Arc};

use itertools::Itertools;

use crate::{
    background_tasks::TagManagerWorker, bot::BotError, database::Database, qdrant::VectorDatabase,
};

use super::{
    image_tag_similarity::convert_vectordb_recommended_tags_to_suggestions, ScoredTagSuggestion,
};

#[tracing::instrument(skip(database, vector_db, tag_manager))]
pub async fn suggest_similar_tags(
    database: &Database,
    vector_db: &VectorDatabase,
    tag_manager: TagManagerWorker,
    tags: &[String],
) -> Result<Vec<ScoredTagSuggestion>, BotError> {
    if tags.len() < 2 || tags.len() >= 30 {
        return Ok(vec![]);
    }
    let result = vector_db.recommend_tags_from_existing_tags(tags).await?;
    if let Some(result) = result {
        Ok(convert_vectordb_recommended_tags_to_suggestions(result, tag_manager).await?)
    } else {
        Ok(vec![])
    }
}
