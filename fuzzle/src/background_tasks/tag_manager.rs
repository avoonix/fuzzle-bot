use chrono::{DateTime, Utc};
use itertools::Itertools;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError, RwLock},
};
use tracing::warn;

use tokio::sync::{mpsc, oneshot};

use crate::{
    Config,
    bot::InternalError,
    database::Database,
    tags::{
        Category, DatabaseTags, E621Tags, ScoredTagSuggestion, TagManager2, TagRepository, Tfidf,
    },
    util::Emoji,
};

#[derive(Clone)]
pub struct TagManagerService {
    // TODO: rename
    tag_manager: Arc<RwLock<TagManager2>>,
    database: Database,
    config: Arc<Config>,
}

impl TagManagerService {
    #[tracing::instrument(skip(database, config))]
    pub async fn new(database: Database, config: Arc<Config>) -> Result<Self, InternalError> {
        let db = DatabaseTags::new(database.clone()).await?;
        let e6 = E621Tags::new(config.tag_cache()).await?;

        let thread_span = tracing::info_span!("spawn_blocking_new_tag_manager").or_current();
        let tag_manager = tokio::task::spawn_blocking(move || {
            thread_span.in_scope(|| {
                Arc::new(RwLock::new(TagManager2::new(vec![
                    TagRepository::DatabaseTags(db),
                    TagRepository::E621Tags(e6),
                ])))
            })
        })
        .await?;

        let manager = Self {
            tag_manager,
            database,
            config,
        };
        Ok(manager)
    }

    /// waits for completion
    #[tracing::instrument(skip(self))]
    pub async fn recompute(&self) -> Result<(), InternalError> {
        let db = DatabaseTags::new(self.database.clone()).await?;
        let e6 = E621Tags::new(self.config.tag_cache()).await?;
        let thread_span = tracing::info_span!("spawn_blocking_new_tag_manager").or_current();
        let mut tag_manager = tokio::task::spawn_blocking(move || {
            thread_span.in_scope(|| {
                TagManager2::new(vec![
                    TagRepository::DatabaseTags(db),
                    TagRepository::E621Tags(e6),
                ])
            })
        })
        .await?;
        {
            let mut m = self.tag_manager.write().unwrap();
            *m = tag_manager;
        }
        Ok(())
    }

    #[must_use]
    #[tracing::instrument(skip(self))]
    pub async fn find_tags(&self, query: &[String]) -> Vec<String> {
        let tag_manager = self.tag_manager.clone();
        let query = query.to_vec();
        let thread_span = tracing::info_span!("spawn_blocking_find_tags").or_current();
        tokio::task::spawn_blocking(move || {
            thread_span.in_scope(|| tag_manager.read().unwrap().find_tags(&query))
        })
        .await
        .unwrap()
    }

    #[must_use]
    pub fn get_category(&self, tag: &str) -> Option<Category> {
        self.tag_manager.read().unwrap().get_category(tag)
    }

    #[must_use]
    pub fn get_implications(&self, tag: &str) -> Option<Vec<String>> {
        self.tag_manager.read().unwrap().get_implications(tag)
    }

    #[must_use]
    pub fn get_implications_including_self(&self, tag: &str) -> Option<Vec<String>> {
        self.tag_manager
            .read()
            .unwrap()
            .get_implications_including_self(tag)
    }

    #[must_use]
    pub fn get_inverse_implications(&self, tag: &str) -> Option<Vec<String>> {
        self.tag_manager
            .read()
            .unwrap()
            .get_inverse_implications(tag)
    }

    #[must_use]
    #[tracing::instrument(skip(self))]
    pub async fn closest_matching_tags(&self, query: &[String]) -> Vec<(String, Option<String>)> {
        let tag_manager = self.tag_manager.clone();
        let query = query.to_vec();
        let thread_span = tracing::info_span!("spawn_blocking_closest_matching_tags").or_current();
        tokio::task::spawn_blocking(move || {
            thread_span.in_scope(|| tag_manager.read().unwrap().closest_matching_tags(&query))
        })
        .await
        .unwrap()
    }

    #[must_use]
    #[tracing::instrument(skip(self))]
    pub async fn closest_matching_tag(&self, query: &str) -> Option<String> {
        let tag_manager = self.tag_manager.clone();
        let query = query.to_string();
        let thread_span = tracing::info_span!("spawn_blocking_closest_matching_tag").or_current();
        tokio::task::spawn_blocking(move || {
            thread_span.in_scope(|| tag_manager.read().unwrap().closest_matching_tag(&query))
        })
        .await
        .unwrap()
    }

    #[must_use]
    pub fn get_tags(&self) -> Vec<String> {
        self.tag_manager.read().unwrap().get_tags()
    }

    #[must_use]
    pub fn get_aliases(&self) -> Vec<String> {
        self.tag_manager.read().unwrap().get_aliases()
    }
}
