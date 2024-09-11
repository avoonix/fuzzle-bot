use std::{collections::HashMap, sync::Arc};

use itertools::Itertools;

use crate::{background_tasks::{GetImplications, TagManagerWorker}, bot::InternalError};

#[derive(Debug, Clone)]
pub struct ScoredTagSuggestion {
    pub tag: String,
    pub score: f64,
}

impl ScoredTagSuggestion {
    #[must_use]
    pub const fn new(tag: String, score: f64) -> Self {
        Self { tag, score }
    }

    // merges two lists of suggestions, adding scores for tags that are in both lists
    #[must_use]
    pub fn merge(suggestions_a: Vec<Self>, suggestions_b: Vec<Self>) -> Vec<Self> {
        suggestions_a
            .into_iter()
            .chain(suggestions_b)
            .sorted_by(|a, b| a.tag.cmp(&b.tag))
            .chunk_by(|suggestion| suggestion.tag.clone())
            .into_iter()
            .map(|(tag, group)| {
                let score = group.map(|suggestion| suggestion.score).sum();
                Self { tag, score }
            })
            .sorted_by(|a, b| b.score.total_cmp(&a.score))
            .collect_vec()
    }

    #[must_use]
    pub async fn add_implications(suggestions: Vec<Self>, tag_manager: TagManagerWorker) -> Result<Vec<Self>, InternalError> {
        let mut map: HashMap<_, _> = suggestions
            .clone()
            .into_iter()
            .map(|s| (s.tag, s.score))
            .collect();
        for suggestion in suggestions {
            let Some(implications) = tag_manager.execute(GetImplications::new(suggestion.tag.clone())).await? else {
                continue;
            };
            let implication_max_score = suggestion.score * 0.9;
            for implication in implications {
                map.entry(implication)
                    .and_modify(|s| *s = s.max(implication_max_score))
                    .or_insert(implication_max_score);
            }
        }
        Ok(map.into_iter().map(|(tag, score)| ScoredTagSuggestion {score, tag}).collect_vec())
    }
}
