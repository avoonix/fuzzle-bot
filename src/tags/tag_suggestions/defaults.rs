use itertools::Itertools;

use super::ScoredTagSuggestion;

#[must_use]
pub fn suggest_default_tags() -> Vec<ScoredTagSuggestion> {
    let common_tags = ["young", "gore", "scat", "watersports", "diaper", "vore"];
    common_tags
        .into_iter()
        .map(|tag| ScoredTagSuggestion {
            tag: tag.into(),
            score: 0.1,
        })
        .collect_vec()
}
