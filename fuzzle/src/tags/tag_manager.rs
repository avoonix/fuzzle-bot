use enum_primitive_derive::Primitive;
use itertools::Itertools;

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    time::SystemTime,
};
use strsim::sorensen_dice;
use tracing::{info, warn};

use crate::{
    bot::InternalError,
    database::{Database, StringVec, Tag},
};

use super::{
    csv::{TagAliasCsv, TagCsv, TagImplicationCsv},
    e621_tags::E621Tags,
    Category,
};

use tag_search_engine::TagSearchEngine;

const MATCH_DISTANCE: f64 = 0.7;
// TODO: rwlock for e621_tags?

pub struct DatabaseTags {
    tags: Vec<Tag>,
}

impl DatabaseTags {
    pub async fn new(db: Database) -> Result<Self, InternalError> {
        let tags = db.get_all_tags().await?;
        Ok(Self { tags })
    }
}

impl DatabaseTags {
    pub fn get_implications(&self) -> HashMap<String, Vec<String>> {
        self.tags
            .iter()
            .map(|tag| {
                (
                    tag.id.clone(),
                    match tag.implications {
                        Some(ref tags) => tags.clone().into_inner(),
                        None => vec![],
                    },
                )
            })
            .collect()
    }

    pub fn get_tags(&self) -> HashMap<String, Category> {
        self.tags
            .iter()
            .map(|tag| (tag.id.clone(), tag.category))
            .collect()
    }

    pub fn get_aliases(&self) -> HashMap<String, String> {
        self.tags
            .iter()
            .flat_map(|tag| match tag.aliases {
                Some(ref tags) => tags
                    .clone()
                    .into_inner()
                    .into_iter()
                    .map(|alias| (alias, tag.id.clone()))
                    .collect_vec(),
                None => vec![],
            })
            .collect()
    }
}

pub enum TagRepository {
    DatabaseTags(DatabaseTags),
    E621Tags(E621Tags),
}

impl TagRepository {
    fn get_implications(&self) -> HashMap<String, Vec<String>> {
        match self {
            TagRepository::DatabaseTags(dt) => dt.get_implications(),
            TagRepository::E621Tags(et) => et.get_implications(),
        }
    }
    fn get_tags(&self) -> HashMap<String, Category> {
        match self {
            TagRepository::DatabaseTags(dt) => dt.get_tags(),
            TagRepository::E621Tags(et) => et.get_tags(),
        }
    }
    fn get_aliases(&self) -> HashMap<String, String> {
        match self {
            TagRepository::DatabaseTags(dt) => dt.get_aliases(),
            TagRepository::E621Tags(et) => et.get_aliases(),
        }
    }
}

pub struct TagManager2 {
    tags: HashMap<String, Category>,
    aliases: HashMap<String, String>,
    implications: HashMap<String, Vec<String>>,
    inverse_implications: HashMap<String, Vec<String>>,
    engine: TagSearchEngine,
}

impl TagManager2 {
    pub fn new(repositories: Vec<TagRepository>) -> Self {
        info!("setting up default tag manager");

        let engine = TagSearchEngine::new(
            &repositories
                .iter()
                .flat_map(|r| {
                    let tags = r.get_tags().keys().map(|term| term.to_string()).collect_vec();
                    let aliases = r.get_aliases().keys().map(|term| term.to_string()).collect_vec();
                    tags.into_iter().chain(aliases.into_iter()).collect_vec()
                })
                .collect_vec(),
        );

        let mut tag_manager = Self {
            tags: HashMap::new(),
            aliases: HashMap::new(),
            implications: HashMap::new(),
            inverse_implications: HashMap::new(),
            engine,
        };

        for repository in repositories {
            tag_manager.aliases.extend(repository.get_aliases());
            tag_manager.tags.extend(repository.get_tags());
            for (tag, implications) in repository.get_implications() {
                for implication in implications {
                    let tag_manager_implications =
                        tag_manager.implications.entry(tag.clone()).or_default();
                    if !tag_manager_implications.contains(&implication) {
                        tag_manager_implications.push(implication);
                    }
                }
            }
        }

        tag_manager
            .compute_transitive_implications()
            .compute_inverse_implications()
    }

    #[must_use]
    pub fn compute_transitive_implications(mut self) -> Self {
        let mut implications = self.implications.clone();
        let mut has_changed = true;
        while has_changed {
            has_changed = false;
            let old_implications = implications.clone();
            for (tag, implied_tags) in &mut implications {
                for implied_tag in implied_tags.clone() {
                    let transitive_implications = old_implications.get(&implied_tag);
                    if let Some(transitive_implications) = transitive_implications {
                        for transitive_implication in transitive_implications {
                            if !implied_tags.contains(transitive_implication) {
                                implied_tags.push(transitive_implication.clone());
                                has_changed = true;
                            }
                        }
                    }
                }
            }
        }
        self.implications = implications;
        self
    }

    #[must_use]
    pub fn compute_inverse_implications(mut self) -> Self {
        let mut inverse_implications = self.inverse_implications.clone();

        for (antecedent, consequents) in &self.implications {
            for consequent in consequents {
                inverse_implications
                    .entry(consequent.to_string())
                    .or_default()
                    .push(antecedent.to_string());
            }
        }
        for consequents in inverse_implications.values_mut() {
            consequents.sort();
            consequents.dedup();
        }

        self.inverse_implications = inverse_implications;
        self
    }
}

impl TagManager2 {
    #[tracing::instrument(skip(entries))]
    fn get_tags_with_similarities<'a>(
        entries: impl Iterator<Item = (&'a String, &'a String)>,
        query: &'a str,
    ) -> impl Iterator<Item = (String, f64)> + 'a {
        entries
            .map(|(alias, tag)| {
                (
                    alias.to_string(),
                    tag.to_string(),
                    sorensen_dice(query, alias),
                )
            })
            .sorted_by(|(alias_a, _, score_a), (alias_b, _, score_b)| {
                score_b
                    .total_cmp(score_a)
                    .then_with(|| alias_a.len().cmp(&alias_b.len()))
            })
            .unique_by(|(_, tag, _)| tag.to_string())
            .map(|(_, tag, score)| (tag, score))
    }

    #[must_use]
    #[tracing::instrument(skip(self))]
    pub fn find_tags(&self, query: &[String]) -> Vec<String> {
        let raw_results = self.engine.search(&query.join(" "));
        let mut results = Vec::new();
        for r in raw_results {
            let r = self.resolve_exact(&r);
            if let Some(r) = r {
                if !results.contains(&r) {
                    results.push(r)
                }
            }
        }
        results
    }

    fn resolve_exact(&self, query: &str) -> Option<String> {
        if self.tags.get(query).is_some() {
            return Some(query.to_string());
        }
        if let Some(tag) = self.aliases.get(query) {
            return Some(tag.to_string());
        }
        None
    }

    #[must_use]
    pub fn get_category(&self, tag: &str) -> Option<Category> {
        self.tags.get(tag).map(std::borrow::ToOwned::to_owned)
    }

    /// gets the implied tags; empty vec if the tag exists; none if the tag does not exist
    #[must_use]
    pub fn get_implications(&self, tag: &str) -> Option<Vec<String>> {
        self.implications
            .get(tag)
            .map(std::borrow::ToOwned::to_owned)
            .or_else(|| self.tags.get(tag).map(|_| Vec::default()))
    }

    #[must_use]
    pub fn get_implications_including_self(&self, tag: &str) -> Option<Vec<String>> {
        self.implications
            .get(tag)
            .map(std::borrow::ToOwned::to_owned)
            .or_else(|| self.tags.get(tag).map(|_| Vec::default()))
            .map(|tags| {
                tags.into_iter()
                    .chain(std::iter::once(tag.to_string()))
                    .collect_vec()
            })
    }

    #[must_use]
    pub fn get_inverse_implications(&self, tag: &str) -> Option<Vec<String>> {
        self.inverse_implications
            .get(tag)
            .map(std::borrow::ToOwned::to_owned)
    }

    /// discards any tags where we couldnt find a match for
    #[must_use]
    #[tracing::instrument(skip(self))]
    pub fn closest_matching_tags(&self, query: &[String]) -> Vec<(String, Option<String>)> {
        query
            .iter()
            .map(|q| (q.clone(), self.closest_matching_tag(q)))
            .collect_vec()
    }

    /// Returns the tag that most closely matches the query
    /// This is used for resolving each tag in a query to one that actually exists
    #[must_use]
    #[tracing::instrument(skip(self))]
    pub fn closest_matching_tag(&self, query: &str) -> Option<String> {
        let res = self.engine.closest(query, MATCH_DISTANCE);
        res.and_then(|r| self.resolve_exact(&r))
    }

    pub fn get_tags(&self) -> Vec<String> {
        self.tags.keys().cloned().collect_vec()
    }

    pub fn get_aliases(&self) -> Vec<String> {
        self.aliases.keys().cloned().collect_vec()
    }
}
