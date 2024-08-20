use std::{collections::HashMap, sync::Arc};

use itertools::Itertools;

use crate::{bot::BotError, database::Database, qdrant::VectorDatabase, tags::TagManager};

use super::{
    image_tag_similarity::convert_vectordb_recommended_tags_to_suggestions, ScoredTagSuggestion,
};

#[tracing::instrument(skip(database, vector_db, tag_manager))]
pub async fn suggest_similar_tags(
    database: &Database,
    vector_db: &VectorDatabase,
    tag_manager: Arc<TagManager>,
    tags: &[String],
) -> Result<Vec<ScoredTagSuggestion>, BotError> {
    if tags.len() < 2 || tags.len() >= 30 {
        return Ok(vec![]);
    }
    let result = vector_db.recommend_tags_from_existing_tags(tags).await?;
    Ok(result.map_or_else(
        || vec![],
        |result| convert_vectordb_recommended_tags_to_suggestions(result, tag_manager),
    ))
}
