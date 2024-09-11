use itertools::Itertools;

use crate::{bot::BotError, database::Database};

use super::ScoredTagSuggestion;

#[tracing::instrument(skip(database))]
pub async fn suggest_owners_tags(
    database: &Database,
    owner_id: i64,
) -> Result<Vec<ScoredTagSuggestion>, BotError> {
    let tags = database.get_all_tags_by_linked_user_id(owner_id).await?;
    Ok(tags
        .into_iter()
        .map(|tag| ScoredTagSuggestion::new(tag.id, 1.0))
        .collect_vec())
}
