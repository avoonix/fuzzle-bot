use itertools::Itertools;

#[derive(Debug, Clone)]
pub struct ScoredTagSuggestion {
    pub tag: String,
    pub score: f64,
}

impl ScoredTagSuggestion {
    #[must_use] pub const fn new(tag: String, score: f64) -> Self {
        Self { tag, score }
    }

    // merges two lists of suggestions, adding scores for tags that are in both lists
    #[must_use] pub fn merge(suggestions_a: Vec<Self>, suggestions_b: Vec<Self>) -> Vec<Self> {
        suggestions_a.into_iter()
            .chain(suggestions_b)
            .sorted_by(|a, b| a.tag.cmp(&b.tag))
            .group_by(|suggestion| suggestion.tag.clone())
            .into_iter()
            .map(|(tag, group)| {
                let score = group.map(|suggestion| suggestion.score).sum();
                Self { tag, score }
            })
            .sorted_by(|a, b| b.score.total_cmp(&a.score))
            .collect_vec()
    }
}

