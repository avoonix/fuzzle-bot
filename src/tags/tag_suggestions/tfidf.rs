

use crate::database::Database;


use crate::util::Emoji;
use anyhow::Result;
use itertools::Itertools;


use std::collections::HashMap;


use super::{ScoredTagSuggestion};


pub struct Tfidf {
    lookup: HashMap<Emoji, Vec<(String, f32)>>,
}

impl Tfidf {
    pub async fn generate(database: Database) -> Result<Self> {
        type Document = Emoji;
        type Term = String;
        let all_used_tags: Vec<(Document, Term, u64)> = database.get_all_tag_emoji_pairs().await?;
        let mut documents: HashMap<Document, HashMap<Term, u64>> = HashMap::new();
        let mut terms: Vec<Term> = Vec::new();
        for (document, term, count) in all_used_tags {
            terms.push(term.clone());
            *documents
                .entry(document)
                .or_default()
                .entry(term)
                .or_default() += count;
        }

        let tf = |term: Term, document: HashMap<Term, u64>| {
            *document.get(&term).unwrap_or(&0) as f32 / document.values().sum::<u64>() as f32
        };

        let mut document_counts: HashMap<Term, f32> = HashMap::new();
        for document in documents.values() {
            for document in document.keys() {
                *document_counts.entry(document.clone()).or_default() += 1.0;
            }
        }
        let documents_2 = documents.clone();
        let idf = |term: Term| {
            (documents.len() as f32 / document_counts.get(&term).unwrap_or(&1.0)).log10()
        };

        let tfidf =
            |term: Term, document: HashMap<Term, u64>| tf(term.clone(), document) * idf(term);

        // computation
        let mut lookup: HashMap<Document, Vec<(Term, f32)>> = HashMap::new();
        for term in terms {
            let mut list: Vec<(Document, f32)> = Vec::new();
            for document in documents_2.clone() {
                let tfidf = tfidf;
                let weight = tfidf(term.clone(), document.1);
                if weight > 0.0 {
                    list.push((document.0, weight));
                }
            }
            let list = list
                .into_iter()
                .sorted_by(|a, b| b.1.total_cmp(&a.1))
                .collect_vec();
            // TODO: take top n list entries
            for list_entry in list {
                let entry = lookup.entry(list_entry.0).or_default();
                if !entry.iter().any(|t| t.0 == term) {
                    entry.push((term.clone(), list_entry.1));
                }
            }
        }
        Ok(Self { lookup })
    }

    pub fn suggest_tags(&self, query: Emoji) -> Vec<ScoredTagSuggestion> {
        let result = self
            .lookup
            .get(&query)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .sorted_by(|a, b| b.1.total_cmp(&a.1))
            .map(|entry| ScoredTagSuggestion {
                score: f64::from(entry.1),
                tag: entry.0,
            })
            .collect_vec();
        let max = result
            .iter()
            .map(|e| e.score)
            .reduce(f64::max)
            .unwrap_or(1.0);
        result
            .into_iter()
            .map(|e| ScoredTagSuggestion {
                score: e.score / max * 0.7,
                tag: e.tag,
            })
            .collect_vec()
    }
}
