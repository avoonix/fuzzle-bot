use itertools::Itertools;

use crate::{bot::BotError, database::Database};

use super::ScoredTagSuggestion;

#[tracing::instrument(skip(database))]
pub async fn suggest_tags_from_same_set(
    database: &Database,
    set_id: &str,
) -> Result<Vec<ScoredTagSuggestion>, BotError> {
    let suggested_tags = database
        .get_all_sticker_set_tag_counts(set_id)
        .await?;
    let max_count = suggested_tags
        .iter()
        .map(|tag| tag.1)
        .max()
        .unwrap_or(1);
    Ok(suggested_tags
        .into_iter()
        .map(|tag| ScoredTagSuggestion::new(tag.0, tag.1 as f64 / max_count as f64))
        .collect_vec())
}
