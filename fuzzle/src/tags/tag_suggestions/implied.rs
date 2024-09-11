use std::sync::Arc;

use futures::future::try_join_all;
use itertools::Itertools;

use crate::{background_tasks::{GetInverseImplications, TagManagerWorker}, bot::InternalError};

use super::tag_suggestion::ScoredTagSuggestion;

#[must_use]
#[tracing::instrument(skip(tag_manager))]
pub async fn suggest_tags_by_reverse_implication(
    known_good_tags: &[String],
    tag_manager: TagManagerWorker,
) -> Result<Vec<ScoredTagSuggestion>, InternalError> {
    Ok(try_join_all(known_good_tags
        .iter()
        .map(|tag| async {
            Ok::<_, InternalError>(tag_manager
            .execute(GetInverseImplications::new(tag.to_string())).await?
                .unwrap_or_default())
        })).await?
        .into_iter()
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
