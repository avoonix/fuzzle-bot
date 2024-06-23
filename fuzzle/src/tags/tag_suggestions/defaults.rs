use itertools::Itertools;

use super::ScoredTagSuggestion;

#[must_use]
pub fn suggest_default_tags() -> Vec<ScoredTagSuggestion> {
    let common_tags = [
        "questionable",
        "explicit",
        "safe",
        "solo",
        "diaper",
        "duo",
        "watersports",
        "young",
        "vore",
        "scat",
        "gore",
    ];
    common_tags
        .into_iter()
        .enumerate()
        .map(|(idx, tag)| ScoredTagSuggestion {
            tag: tag.into(),
            score: -(idx as f64),
        })
        .collect_vec()
}
