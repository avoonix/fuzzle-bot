use std::sync::Arc;

use itertools::Itertools;

use crate::tags::TagManager;

use super::tag_suggestion::ScoredTagSuggestion;

#[must_use]
pub fn suggest_tags_by_reverse_implication(
    known_good_tags: &[String],
    tag_manager: Arc<TagManager>,
) -> Vec<ScoredTagSuggestion> {
    known_good_tags
        .iter()
        .flat_map(|tag| {
            tag_manager
                .get_inverse_implications(tag)
                .unwrap_or_default()
        })
        .sorted()
        .group_by(std::clone::Clone::clone)
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
        .collect_vec()
}

const fn compute_score_for_implication_count(count: usize) -> f64 {
    match count {
        1 => 0.1,
        2 => 0.6,
        3 => 0.8,
        4 => 0.9,
        _ => 0.99,
    }
}
