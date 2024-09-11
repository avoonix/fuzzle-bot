use chrono::{DateTime, Utc};
use itertools::Itertools;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};
use tracing::warn;

use tokio::sync::{mpsc, oneshot};

use crate::{
    bot::InternalError,
    database::Database,
    tags::{Category, DatabaseTags, E621Tags, ScoredTagSuggestion, TagManager2, TagRepository, Tfidf},
    util::Emoji,
    Config,
};

use super::{Comm, State, Worker};

#[derive(Clone, Debug)]
pub struct TagManagerState {
    // lookup: HashMap<u64, String>,
    tag_manager: TagManager2,
    last_computed: DateTime<Utc>,
    // database: Database, // TODO: dont put in state
}

#[derive(Clone)]
pub struct TagManagerDependencies {
    // TODO: database, config
    pub database: Database,
    pub config: Arc<Config>,
}

impl TagManagerState {
    #[tracing::instrument(skip(deps))]
    async fn new(deps: TagManagerDependencies) -> Result<Arc<Self>, InternalError> {
        // let analysis = database.get_analysis_for_all_stickers_with_tags().await?;
        // let (embedding_index, color_index, _, lookup) =
        //     tokio::task::spawn_blocking(move || recompute_index(analysis)).await??;
        // let all_used_tags: Vec<(Emoji, String, i64)> = deps.database.get_all_tag_emoji_pairs().await?;
        // let tfidf = tokio::task::spawn_blocking(move || Arc::new(Tfidf::generate(all_used_tags))).await?;
        let last_computed = chrono::Utc::now();

        let db = DatabaseTags::new(deps.database).await?;
        let e6 = E621Tags::new(deps.config.tag_cache()).await?;

        let tag_manager = TagManager2::new(vec![TagRepository::DatabaseTags(db), TagRepository::E621Tags(e6)]);

        Ok(Arc::new(Self {
            // embedding_index,
            // color_index,
            // lookup,
            tag_manager,
            last_computed,
        }))
    }
    async fn needs_recomputation(
        &self,
        deps: TagManagerDependencies,
    ) -> Result<bool, InternalError> {
        Ok(chrono::Utc::now() - self.last_computed > chrono::Duration::hours(4))
    }
}

impl State<TagManagerDependencies> for TagManagerState {
    fn generate(
        deps: TagManagerDependencies,
    ) -> impl std::future::Future<Output = Result<Arc<Self>, InternalError>> + Send {
        Self::new(deps)
    }
    fn needs_recomputation(
        &self,
        deps: TagManagerDependencies,
    ) -> impl std::future::Future<Output = Result<bool, InternalError>> + Send {
        self.needs_recomputation(deps)
    }
}

pub type TagManagerWorker = Worker<TagManagerState, TagManagerDependencies>;

pub struct GetImplicationsIncludingSelf {
    tag: String,
}

impl GetImplicationsIncludingSelf {
    pub fn new(tag: String) -> Self {
        Self { tag }
    }

    #[tracing::instrument(skip(self, state, deps))]
    async fn apply(
        &self,
        state: Arc<TagManagerState>,
        deps: TagManagerDependencies,
    ) -> Result<Option<Vec<String>>, InternalError> {
        Ok(state.tag_manager.get_implications_including_self(&self.tag))
    }
}

impl Comm<TagManagerState, TagManagerDependencies> for GetImplicationsIncludingSelf {
    type ReturnType = Option<Vec<String>>;
    fn apply(
        &self,
        state: Arc<TagManagerState>,
        deps: TagManagerDependencies,
    ) -> impl std::future::Future<Output = Result<Self::ReturnType, InternalError>> + Send {
        Self::apply(&self, state, deps)
    }
}

pub struct GetCategory {
    tag: String,
}

impl GetCategory {
    pub fn new(tag: String) -> Self {
        Self { tag }
    }

    #[tracing::instrument(skip(self, state, deps))]
    async fn apply(
        &self,
        state: Arc<TagManagerState>,
        deps: TagManagerDependencies,
    ) -> Result<Option<Category>, InternalError> {
        Ok(state.tag_manager.get_category(&self.tag))
    }
}

impl Comm<TagManagerState, TagManagerDependencies> for GetCategory {
    type ReturnType = Option<Category>;
    fn apply(
        &self,
        state: Arc<TagManagerState>,
        deps: TagManagerDependencies,
    ) -> impl std::future::Future<Output = Result<Self::ReturnType, InternalError>> + Send {
        Self::apply(&self, state, deps)
    }
}

pub struct GetImplications {
    tag: String,
}

impl GetImplications {
    pub fn new(tag: String) -> Self {
        Self { tag }
    }

    #[tracing::instrument(skip(self, state, deps))]
    async fn apply(
        &self,
        state: Arc<TagManagerState>,
        deps: TagManagerDependencies,
    ) -> Result<Option<Vec<String>>, InternalError> {
        Ok(state.tag_manager.get_implications(&self.tag))
    }
}

impl Comm<TagManagerState, TagManagerDependencies> for GetImplications {
    type ReturnType = Option<Vec<String>>;
    fn apply(
        &self,
        state: Arc<TagManagerState>,
        deps: TagManagerDependencies,
    ) -> impl std::future::Future<Output = Result<Self::ReturnType, InternalError>> + Send {
        Self::apply(&self, state, deps)
    }
}

pub struct ClosestMatchingTags {
    query: Vec<String>,
}

impl ClosestMatchingTags {
    pub fn new(query: Vec<String>) -> Self {
        Self { query }
    }

    #[tracing::instrument(skip(self, state, deps))]
    async fn apply(
        &self,
        state: Arc<TagManagerState>,
        deps: TagManagerDependencies,
    ) -> Result<Vec<(String, Option<String>)>, InternalError> {
        Ok(state.tag_manager.closest_matching_tags(&self.query))
    }
}

impl Comm<TagManagerState, TagManagerDependencies> for ClosestMatchingTags {
    type ReturnType = Vec<(String, Option<String>)>;
    fn apply(
        &self,
        state: Arc<TagManagerState>,
        deps: TagManagerDependencies,
    ) -> impl std::future::Future<Output = Result<Self::ReturnType, InternalError>> + Send {
        Self::apply(&self, state, deps)
    }
}

pub struct FindTags {
    query: Vec<String>,
}

impl FindTags {
    pub fn new(query: Vec<String>) -> Self {
        Self { query }
    }

    #[tracing::instrument(skip(self, state, deps))]
    async fn apply(
        &self,
        state: Arc<TagManagerState>,
        deps: TagManagerDependencies,
    ) -> Result<Vec<String>, InternalError> {
        Ok(state.tag_manager.find_tags(&self.query))
    }
}

impl Comm<TagManagerState, TagManagerDependencies> for FindTags {
    type ReturnType = Vec<String>;
    fn apply(
        &self,
        state: Arc<TagManagerState>,
        deps: TagManagerDependencies,
    ) -> impl std::future::Future<Output = Result<Self::ReturnType, InternalError>> + Send {
        Self::apply(&self, state, deps)
    }
}

pub struct ClosestMatchingTag {
    query: String,
}

impl ClosestMatchingTag {
    pub fn new(query: String) -> Self {
        Self { query }
    }

    #[tracing::instrument(skip(self, state, deps))]
    async fn apply(
        &self,
        state: Arc<TagManagerState>,
        deps: TagManagerDependencies,
    ) -> Result<Option<String>, InternalError> {
        Ok(state.tag_manager.closest_matching_tag(&self.query))
    }
}

impl Comm<TagManagerState, TagManagerDependencies> for ClosestMatchingTag {
    type ReturnType = Option<String>;
    fn apply(
        &self,
        state: Arc<TagManagerState>,
        deps: TagManagerDependencies,
    ) -> impl std::future::Future<Output = Result<Self::ReturnType, InternalError>> + Send {
        Self::apply(&self, state, deps)
    }
}

pub struct GetInverseImplications {
    tag: String,
}

impl GetInverseImplications {
    pub fn new(tag: String) -> Self {
        Self { tag }
    }

    #[tracing::instrument(skip(self, state, deps))]
    async fn apply(
        &self,
        state: Arc<TagManagerState>,
        deps: TagManagerDependencies,
    ) -> Result<Option<Vec<String>>, InternalError> {
        Ok(state.tag_manager.get_inverse_implications(&self.tag))
    }
}

impl Comm<TagManagerState, TagManagerDependencies> for GetInverseImplications {
    type ReturnType = Option<Vec<String>>;
    fn apply(
        &self,
        state: Arc<TagManagerState>,
        deps: TagManagerDependencies,
    ) -> impl std::future::Future<Output = Result<Self::ReturnType, InternalError>> + Send {
        Self::apply(&self, state, deps)
    }
}

pub struct GetTagsAndAliases {}

pub struct TagsAndAliases {
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
}

impl GetTagsAndAliases {
    pub fn new() -> Self {
        Self {}
    }

    #[tracing::instrument(skip(self, state, deps))]
    async fn apply(
        &self,
        state: Arc<TagManagerState>,
        deps: TagManagerDependencies,
    ) -> Result<TagsAndAliases, InternalError> {
        Ok(TagsAndAliases {
            tags: state.tag_manager.get_tags(),
            aliases: state.tag_manager.get_aliases(),
        })
    }
}

impl Comm<TagManagerState, TagManagerDependencies> for GetTagsAndAliases {
    type ReturnType = TagsAndAliases;
    fn apply(
        &self,
        state: Arc<TagManagerState>,
        deps: TagManagerDependencies,
    ) -> impl std::future::Future<Output = Result<Self::ReturnType, InternalError>> + Send {
        Self::apply(&self, state, deps)
    }
}
