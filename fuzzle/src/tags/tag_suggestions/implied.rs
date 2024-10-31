use std::sync::Arc;

use itertools::Itertools;

use crate::{background_tasks::TagManagerService, bot::InternalError};

use super::tag_suggestion::ScoredTagSuggestion;

#[must_use]
#[tracing::instrument(skip(tag_manager))]
pub async fn suggest_tags_by_reverse_implication(
    known_good_tags: &[String],
    tag_manager: TagManagerService,
) -> Result<Vec<ScoredTagSuggestion>, InternalError> {
    Ok(known_good_tags
        .iter()
        .map(|tag| {
            tag_manager
                .get_inverse_implications(&tag)
                .unwrap_or_default()
        })
        .flatten()
        .sorted()
        .chunk_by(std::clone::Clone::clone)
        .into_iter()
        .map(|(tag, group)| ScoredTagSuggestion {
            tag,
            score: compute_score_for_implication_count(group.count()),
        })
        .sorted_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .collect_vec())
}

const fn compute_score_for_implication_count(count: usize) -> f64 {
    match count {
        1 => 0.1,
        2 => 0.3,
        3 => 0.4,
        4 => 0.5,
        _ => 0.6,
    }
}
