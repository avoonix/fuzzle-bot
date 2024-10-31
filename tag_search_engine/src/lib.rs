/*

This is a very bad implementation (but still better than my previous tag search approach) of a trigram similarity search.
Combined with a trie-based prefix search.

Possible ways to improve the search:
- Create a trigram -> int mapping and only work with ints - avoids cloning strings, better use of cache
- Add popularity (sticker count) information to the tags and use that when ranking results

*/

use std::collections::{BTreeSet, HashMap, HashSet};

use itertools::Itertools;
use regex::Regex;
use trie_rs::{Trie, TrieBuilder};

fn jaccard_similarity(s1: &HashSet<String>, s2: &HashSet<String>) -> f64 {
    let i = s1.intersection(s2).count() as f64;
    let u = s1.union(s2).count() as f64;
    if u == 0.0 {
        1.0
    } else {
        i / u
    }
}

fn get_trigrams(s: &str) -> HashSet<String> {
    let re = Regex::new(r"(^|$)").expect("hardcoded regex to compile");
    re.replace_all(s, "__")
        .into_owned()
        .chars()
        .tuple_windows()
        .map(|(c0, c1, c2)| format!("{c0}{c1}{c2}"))
        .collect()
}

fn merge(left: &[(String, usize)], right: &[(String, usize)]) -> Vec<(String, usize)> {
    let mut merged = Vec::new();
    let mut i = 0;
    let mut j = 0;

    while i < left.len() && j < right.len() {
        if left[i].0 < right[j].0 {
            merged.push(left[i].clone());
            i += 1;
        } else if left[i].0 > right[j].0 {
            merged.push(right[j].clone());
            j += 1;
        } else {
            merged.push((left[i].0.clone(), left[i].1 + right[j].1));
            i += 1;
            j += 1;
        }
    }

    while i < left.len() {
        merged.push(left[i].clone());
        i += 1;
    }
    while j < right.len() {
        merged.push(right[j].clone());
        j += 1;
    }

    merged
}

fn merge_1(counts: &[Vec<(String, usize)>]) -> Vec<(String, usize)> {
    if counts.len() == 1 {
        counts[0].to_vec()
    } else if counts.len() == 2 {
        merge(&counts[0], &counts[1])
    } else {
        let mid = counts.len() / 2;
        let (left, right) = counts.split_at(mid);
        let left = merge_1(left);
        let right = merge_1(right);
        merge(&left, &right)
    }
}

fn count_terms(sets: &[&BTreeSet<String>]) -> Vec<(String, usize)> {
    if sets.is_empty() {
        return vec![];
    }
    let counts: Vec<Vec<(String, usize)>> = sets
        .into_iter()
        .map(|set| set.iter().map(|trigram| (trigram.to_string(), 1)).collect())
        .collect();
    merge_1(&counts)
}

fn create_trie(terms: &[String]) -> Trie<u8> {
    let mut builder = TrieBuilder::new();
    for term in terms {
        builder.push(term);
    }
    builder.build()
}

fn get_results_trie(trie: &Trie<u8>, query: &str) -> Vec<(String, f64)> {
    trie.predictive_search(query)
        .map(|res: String| (res.clone(), query.len() as f64 / res.len() as f64))
        .sorted_by_key(|(_, count)| -(*count * 10000.0) as i64)
        .take(20)
        .collect()
}

fn create_trigram_index(
    terms: &[String],
) -> (
    HashMap<String, HashSet<String>>,
    HashMap<String, BTreeSet<String>>,
) {
    let trigrams: HashMap<String, HashSet<String>> = terms
        .into_iter()
        .map(|term| (term.to_string(), get_trigrams(&term)))
        .collect();
    let index: HashMap<_, BTreeSet<_>> = trigrams
        .iter()
        .flat_map(|(term, trigrams)| trigrams.iter().map(|trigram| (trigram, term.clone())))
        .sorted_by_key(|(trigram, _)| trigram.clone())
        .chunk_by(|(trigram, _)| trigram.clone())
        .into_iter()
        .map(|(trigram, terms)| (trigram.to_string(), terms.map(|(_, term)| term).collect()))
        .collect();

    (trigrams, index)
}

pub struct TagSearchEngine {
    trie: Trie<u8>,
    trigrams: HashMap<String, HashSet<String>>,
    index: HashMap<String, BTreeSet<String>>,
    max_len: usize,
}

impl TagSearchEngine {
    pub fn new(terms: &[String]) -> Self {
        let (trigrams, index) = create_trigram_index(terms);
        let trie = create_trie(terms);
        let max_len = terms
            .into_iter()
            .map(|term| term.len())
            .max()
            .unwrap_or_default();
        Self {
            index,
            trie,
            trigrams,
            max_len,
        }
    }

    pub fn search(&self, query: &str) -> Vec<String> {
        if query.is_empty() || query.len() > 2 * self.max_len {
            return vec![];
        }
        let trie = get_results_trie(&self.trie, query);
        let trigram = if query.len() > 1 {
            get_results_trigram_index(&self.trigrams, &self.index, query)
        } else {
            vec![]
        };
        merge_results(trie, trigram)
    }

    pub fn closest(&self, query: &str, max_distance: f64) -> Option<String> {
        if query.is_empty() || query.len() > 2 * self.max_len {
            return None;
        }
        let trie = get_results_trie(&self.trie, query);
        let trigram = if query.len() > 1 {
            get_results_trigram_index(&self.trigrams, &self.index, query)
        } else {
            vec![]
        };
        match (trie.first(), trigram.first()) {
            (None, None) => None,
            (None, Some(f)) | (Some(f), None) => {
                if f.1 >= max_distance {
                    Some(f.0.clone())
                } else {
                    None
                }
            }
            (Some(f1), Some(f2)) => {
                if f1.1 > f2.1 {
                    if f1.1 >= max_distance {
                        Some(f1.0.clone())
                    } else {
                        None
                    }
                } else {
                    if f2.1 >= max_distance {
                        Some(f2.0.clone())
                    } else {
                        None
                    }
                }
            }
        }
    }
}

fn merge_results(
    trie_results: Vec<(String, f64)>,
    trigram_results: Vec<(String, f64)>,
) -> Vec<String> {
    let mut merged = Vec::new();
    let mut i = 0;
    let mut j = 0;

    let mut push = |res: String| {
        if !merged.contains(&res) {
            merged.push(res);
        }
    };

    while i < trie_results.len() && j < trigram_results.len() {
        if trie_results[i].0 < trigram_results[j].0 {
            push(trie_results[i].0.clone());
            i += 1;
        } else if trie_results[i].0 > trigram_results[j].0 {
            push(trigram_results[j].0.clone());
            j += 1;
        } else {
            push(trigram_results[i].0.clone());
            i += 1;
            j += 1;
        }
    }

    while i < trie_results.len() {
        push(trie_results[i].0.clone());
        i += 1;
    }
    while j < trigram_results.len() {
        push(trigram_results[j].0.clone());
        j += 1;
    }

    merged
}

pub fn get_results_trigram_index(
    trigrams: &HashMap<String, HashSet<String>>,
    index: &HashMap<String, BTreeSet<String>>,
    query: &str,
) -> Vec<(String, f64)> {
    let query = get_trigrams(query);
    let min_length_to_consider_match = query.len() / 2;
    let sets = query
        .iter()
        .filter_map(|trigram| index.get(trigram))
        .collect_vec();


    count_terms(&sets)
        .iter()
        .filter(|(_, count)| *count >= min_length_to_consider_match)
        .map(|(term, count)| {
            (
                term.clone(),
                (jaccard_similarity(&query, trigrams.get(term).unwrap())),
            )
        })
        .sorted_by_key(|(_, count)| -(*count * 10000.0) as i64)
        .take(20)
        .collect_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::Itertools;

    #[test]
    fn test_search_engine() {
        let terms = include_str!("../resources/test/popular.txt");
        let engine = TagSearchEngine::new(
            &terms
                .split("\n")
                .map(|s| s.to_string())
                .skip(1)
                .collect_vec(),
        );
        assert_eq!(engine.max_len, 20);
        assert!(engine.search("simil").contains(&"similar".to_string()));
        assert!(engine.search("ap").contains(&"apes".to_string()));
    }
}
