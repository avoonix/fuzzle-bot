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
    tags::{ScoredTagSuggestion, Tfidf},
    util::Emoji,
};

use super::{Comm, State, TagManagerWorker, Worker};

#[derive(Clone, Debug)]
pub struct TfIdfState {
    // lookup: HashMap<u64, String>,
    tfidf: Arc<Tfidf>,
    last_computed: DateTime<Utc>,
    // database: Database, // TODO: dont put in state
}

#[derive(Clone)]
pub struct TfIdfDependencies {
    pub database: Database,
    pub tag_manager: TagManagerWorker,
}

impl TfIdfState {
    #[tracing::instrument(skip(deps))]
    async fn new(deps: TfIdfDependencies) -> Result<Arc<Self>, InternalError> {
        let all_used_tags: Vec<(Emoji, String, i64)> =
            deps.database.get_all_tag_emoji_pairs().await?;
        let tfidf =
            tokio::task::spawn_blocking(move || Arc::new(Tfidf::generate(all_used_tags))).await?;
        let last_computed = chrono::Utc::now();

        Ok(Arc::new(Self {
            tfidf,
            last_computed,
        }))
    }
    async fn needs_recomputation(&self, deps: TfIdfDependencies) -> Result<bool, InternalError> {
        Ok(chrono::Utc::now() - self.last_computed > chrono::Duration::minutes(5))
    }
}

impl State<TfIdfDependencies> for TfIdfState {
    fn generate(
        deps: TfIdfDependencies,
    ) -> impl std::future::Future<Output = Result<Arc<Self>, InternalError>> + Send {
        Self::new(deps)
    }
    fn needs_recomputation(
        &self,
        deps: TfIdfDependencies,
    ) -> impl std::future::Future<Output = Result<bool, InternalError>> + Send {
        self.needs_recomputation(deps)
    }
}

pub type TfIdfWorker = Worker<TfIdfState, TfIdfDependencies>;

pub struct SuggestTags {
    sticker_id: String,
}

impl SuggestTags {
    pub fn new(sticker_id: String) -> Self {
        Self { sticker_id }
    }

    #[tracing::instrument(skip(self, state, deps))]
    async fn apply(
        &self,
        state: Arc<TfIdfState>,
        deps: TfIdfDependencies,
    ) -> Result<Vec<ScoredTagSuggestion>, InternalError> {
        Ok(deps
            .database
            .get_sticker_by_id(&self.sticker_id)
            .await?
            .and_then(|sticker| {
                sticker
                    .emoji
                    .map(|e| Emoji::new_from_string_single(e))
                    .map(|emoji| state.tfidf.suggest_tags(emoji))
            })
            .unwrap_or_default())
    }
}

impl Comm<TfIdfState, TfIdfDependencies> for SuggestTags {
    type ReturnType = Vec<ScoredTagSuggestion>;
    fn apply(
        &self,
        state: Arc<TfIdfState>,
        deps: TfIdfDependencies,
    ) -> impl std::future::Future<Output = Result<Self::ReturnType, InternalError>> + Send {
        Self::apply(&self, state, deps)
    }
}
