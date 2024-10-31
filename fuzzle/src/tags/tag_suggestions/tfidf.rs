use crate::database::Database;

use anyhow::Result;
use itertools::Itertools;

use std::{collections::HashMap, hash::Hash};

use super::ScoredTagSuggestion;

#[derive(Debug)]
pub struct Tfidf<T: Hash + Clone + Eq, D: Hash + Clone + Eq> { // term - document
    lookup: HashMap<D, Vec<ScoredTagSuggestion<T>>>,
}

impl<T: Hash + Clone + Eq, D: Hash + Clone + Eq> Tfidf<T, D> {
    #[tracing::instrument(skip(all_used_tags))]
    pub fn generate(all_used_tags: Vec<(D, T, i64)>) -> Self {
        let mut documents: HashMap<D, HashMap<T, u64>> = HashMap::new();
        let mut terms: Vec<T> = Vec::new();
        for (document, term, count) in all_used_tags {
            terms.push(term.clone());
            *documents
                .entry(document)
                .or_default()
                .entry(term)
                .or_default() += count as u64;
        }

        let tf = |term: T, document: HashMap<T, u64>| {
            *document.get(&term).unwrap_or(&0) as f32 / document.values().sum::<u64>() as f32
        };

        let mut document_counts: HashMap<T, f32> = HashMap::new();
        for document in documents.values() {
            for document in document.keys() {
                *document_counts.entry(document.clone()).or_default() += 1.0;
            }
        }
        let documents_2 = documents.clone();
        let idf = |term: T| {
            (documents.len() as f32 / document_counts.get(&term).unwrap_or(&1.0)).log10()
        };

        let tfidf =
            |term: T, document: HashMap<T, u64>| tf(term.clone(), document) * idf(term);

        // computation
        let mut lookup: HashMap<D, Vec<(T, f32)>> = HashMap::new();
        for term in terms {
            let mut list: Vec<(D, f32)> = Vec::new();
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
        let lookup = lookup
            .into_iter()
            .map(|(k, v)| {
                (
                    k,
                    v.into_iter()
                        .sorted_by(|a, b| b.1.total_cmp(&a.1))
                        .take(30)
                        .map(|entry| ScoredTagSuggestion {
                            score: f64::from(entry.1),
                            tag: entry.0,
                        })
                        .collect_vec(),
                )
            })
            .collect();
        Self { lookup }
    }

    pub fn suggest(&self, query: D) -> Vec<ScoredTagSuggestion<T>> {
        self.lookup.get(&query).cloned().unwrap_or_default()
    }
}
