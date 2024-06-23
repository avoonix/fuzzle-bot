use chrono::{DateTime, Utc};
use itertools::Itertools;
use tracing::warn;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};

use tokio::sync::{mpsc, oneshot};

use crate::{
    bot::BotError,
    database::Database,
    tags::{ScoredTagSuggestion, TagManager, Tfidf}, util::Emoji,
};

use super::{Comm, State, Worker};

#[derive(Clone, Debug)]
pub struct TaggingState {
    // lookup: HashMap<u64, String>,
    tfidf: Arc<Tfidf>,
    last_computed: DateTime<Utc>,
    database: Database, // TODO: dont put in state
}

impl TaggingState {
    #[tracing::instrument(skip(database))]
    async fn new(database: Database) -> Result<Arc<Self>, BotError> {
        // let analysis = database.get_analysis_for_all_stickers_with_tags().await?;
        // let (embedding_index, color_index, _, lookup) =
        //     tokio::task::spawn_blocking(move || recompute_index(analysis)).await??;
        let tfidf = Arc::new(Tfidf::generate(database.clone()).await?);
        let last_computed = chrono::Utc::now();

        Ok(Arc::new(Self {
            // embedding_index,
            // color_index,
            // lookup,
            tfidf,
            last_computed,
            database,
        }))
    }
    async fn needs_recomputation(&self, database: Database) -> Result<bool, BotError> {
        Ok(chrono::Utc::now() - self.last_computed > chrono::Duration::minutes(5))
    }
}

impl State for TaggingState {
    fn generate(
        database: Database,
        _: Arc<TagManager>,
    ) -> impl std::future::Future<Output = Result<Arc<Self>, BotError>> + Send {
        Self::new(database)
    }
    fn needs_recomputation(
        &self,
        database: Database,
    ) -> impl std::future::Future<Output = Result<bool, BotError>> + Send {
        self.needs_recomputation(database)
    }
}

pub type TaggingWorker = Worker<TaggingState>;

pub struct SuggestTags {
    sticker_id: String,
}

impl SuggestTags {
    pub fn new(sticker_id: String) -> Self {
        Self { sticker_id }
    }

    #[tracing::instrument(skip(self, state))]
    async fn apply(&self, state: Arc<TaggingState>) -> Result<Vec<ScoredTagSuggestion>, BotError> {
       
        Ok(state.database
            .get_sticker_by_id(&self.sticker_id)
            .await?
            .and_then(|sticker| {
                sticker.emoji.map(|e| Emoji::new_from_string_single(e)).map(|emoji| state.tfidf.suggest_tags(emoji))
            })
            .unwrap_or_default())
    }
}

impl Comm<TaggingState> for SuggestTags {
    type ReturnType = Vec<ScoredTagSuggestion>;
    fn apply(
        &self,
        state: Arc<TaggingState>,
    ) -> impl std::future::Future<Output = Result<Self::ReturnType, BotError>> + Send {
        Self::apply(&self, state)
    }
}
